"""Exercise a signed-in Codex app server on synthetic text. No fixture files are exposed."""
from pathlib import Path
from queue import Empty, Queue
from tempfile import TemporaryDirectory
from threading import Thread
from time import monotonic
import json
import os
import subprocess


PROMPT = (
    "Respond with only one revised sentence for the supplied selection. Do not call any tools. "
    "Selection: Keep the first release on the current schedule. "
    "Approved reference: The pilot launch moves to 14 December."
)


def check(directory):
    with subprocess.Popen(
        ["codex", "-c", "mcp_servers={}", "--disable", "shell_tool",
         "app-server", "--listen", "stdio://"],
        env={**os.environ, "CODEX_SQLITE_HOME": directory},
        cwd=directory, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL, text=True,
    ) as process:
        responses = Queue()

        def read_responses():
            for line in process.stdout:
                responses.put(json.loads(line))
            responses.put(None)

        Thread(target=read_responses, daemon=True).start()
        deadline = monotonic() + 45
        waiting = []

        def send(method, request=None, **params):
            message = {"method": method, "params": params}
            if request is not None:
                message["id"] = request
            process.stdin.write(json.dumps(message) + "\n")
            process.stdin.flush()

        def await_message(predicate):
            for index, response in enumerate(waiting):
                if predicate(response):
                    return waiting.pop(index)
            while True:
                try:
                    response = responses.get(timeout=max(0, deadline - monotonic()))
                except Empty:
                    raise RuntimeError("Codex app-server probe timed out") from None
                if response is None:
                    raise RuntimeError("Codex app-server exited")
                if response.get("method") and "id" in response:
                    response_id = response["id"]
                    process.stdin.write(json.dumps({
                        "id": response_id, "error": {"code": -32601, "message": "Denied by fixture host"}
                    }) + "\n")
                    process.stdin.flush()
                    continue
                if predicate(response):
                    return response
                waiting.append(response)

        def result(request):
            reply = await_message(lambda response: response.get("id") == request)
            if "error" in reply:
                error = reply["error"]
                raise RuntimeError(f"Codex RPC {request} error {error.get('code')}: {error.get('message')}")
            return reply["result"]

        try:
            send("initialize", 1, clientInfo={"name": "offish_phase0", "version": "0.0.0"})
            result(1)
            send("initialized")
            send("thread/start", 2, cwd=directory, ephemeral=True,
                 sandbox="read-only", approvalPolicy="never")
            thread_id = result(2)["thread"]["id"]
            # This is a synthetic transport trial. Read-only does not constrain reads.
            sandbox = {"type": "readOnly"}
            send("turn/start", 3, threadId=thread_id, sandboxPolicy=sandbox,
                 input=[{"type": "text", "text": PROMPT, "text_elements": []}])
            result(3)
            complete = await_message(lambda response: response.get("method") == "turn/completed")
            turn = complete["params"]["turn"]
            if turn["status"] != "completed":
                info = (turn.get("error") or {}).get("codexErrorInfo")
                if isinstance(info, dict):
                    info = ", ".join(info.keys())
                raise RuntimeError(f"Synthetic model turn {turn['status']}; error category: {info}")
            deltas = [event["params"]["delta"] for event in waiting
                      if event.get("method") == "item/agentMessage/delta"]
            if "14 December" not in "".join(deltas):
                raise RuntimeError("Expected revision was missing from streaming output")
            kinds = [event["params"]["item"]["type"] for event in waiting
                     if event.get("method") == "item/started"]
            if any(kind not in {"userMessage", "agentMessage", "reasoning", "plan"} for kind in kinds):
                raise RuntimeError("The trial attempted to run a tool")
            print(f"PASS: fixture answer via {len(deltas)} streamed text events")
            send("turn/start", 4, threadId=thread_id, sandboxPolicy=sandbox,
                 input=[{"type": "text", "text": "Output a long numbered list without tools.",
                         "text_elements": []}])
            turn_id = result(4)["turn"]["id"]
            send("turn/interrupt", 5, threadId=thread_id, turnId=turn_id)
            result(5)
            complete = await_message(
                lambda response: response.get("method") == "turn/completed"
                and response["params"]["turn"]["id"] == turn_id
            )
            assert complete["params"]["turn"]["status"] == "interrupted"
            print("PASS: turn interrupt acknowledged and final status interrupted")
        finally:
            process.terminate()
            process.wait(timeout=5)


if __name__ == "__main__":
    with TemporaryDirectory(prefix="offish-codex-") as directory:
        check(str(Path(directory).resolve()))
