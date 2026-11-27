export function dialogButtonIndex(current, count, key) {
    if (current < 0 || current >= count || (key !== "ArrowLeft" && key !== "ArrowRight")) return null;
    return Math.max(0, Math.min(count - 1, current + (key === "ArrowLeft" ? -1 : 1)));
}

export function menuPosition(anchor, size, viewport) {
    const gap = 6;
    return {
        left: Math.max(gap, Math.min(anchor.left, viewport.width - size.width - gap)),
        top: Math.max(gap, Math.min(anchor.bottom, viewport.height - size.height - gap)),
    };
}

export function floatingMenu(element, report) {
    let trigger = null;

    function close(focus = false) {
        if (!trigger) return;
        const previous = trigger;
        trigger = null;
        previous.setAttribute("aria-expanded", "false");
        element.hidden = true;
        if (focus && previous.isConnected) previous.focus();
    }

    function open(anchor, actions) {
        if (anchor === trigger) return close(true);
        close();
        trigger = anchor;
        anchor.setAttribute("aria-expanded", "true");
        element.replaceChildren(...actions.map(action => {
            if (action === null) {
                const divider = document.createElement("hr");
                divider.setAttribute("role", "separator");
                return divider;
            }
            const item = document.createElement("button");
            item.type = "button";
            item.setAttribute("role", "menuitem");
            item.textContent = action.label;
            item.disabled = Boolean(action.disabled);
            item.onclick = () => {
                close(true);
                Promise.resolve().then(action.run).catch(report);
            };
            return item;
        }));
        element.setAttribute("aria-label", anchor.getAttribute("aria-label") ?? anchor.textContent);
        element.hidden = false;
        const {left, top} = menuPosition(anchor.getBoundingClientRect(),
            {width: element.offsetWidth, height: element.offsetHeight},
            {width: window.innerWidth, height: window.innerHeight});
        element.style.left = left + "px";
        element.style.top = top + "px";
        (element.querySelector("button:not(:disabled)") ?? element).focus();
    }

    element.addEventListener("keydown", event => {
        const items = [...element.querySelectorAll("button:not(:disabled)")];
        const index = items.indexOf(document.activeElement);
        if (event.key === "Escape" || event.key === "Tab") {
            close(true);
            if (event.key === "Escape") event.preventDefault();
            return;
        }
        if (!items.length) return;
        if (event.key === "ArrowDown" || event.key === "ArrowUp") {
            event.preventDefault();
            const offset = event.key === "ArrowDown" ? 1 : -1;
            items[(index + offset + items.length) % items.length].focus();
        } else if (event.key === "Home" || event.key === "End") {
            event.preventDefault();
            (event.key === "Home" ? items[0] : items.at(-1)).focus();
        }
    });
    document.addEventListener("pointerdown", event => {
        if (trigger && !trigger.contains(event.target) && !element.contains(event.target)) close();
    });
    window.addEventListener("resize", () => close(true));
    document.addEventListener("scroll", event => {
        if (trigger && !element.contains(event.target)) close();
    }, true);
    return {open, close};
}
