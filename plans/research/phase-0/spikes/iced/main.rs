use iced::widget::{column, text, text_editor};
use iced::{Element, Task};

#[derive(Debug, Clone)]
struct Edit(text_editor::Action);

fn update(editor: &mut text_editor::Content, action: Edit) {
    let editing = action.0.is_edit();
    editor.perform(action.0);
    if editing {
        println!("Fixture text: {}", editor.text());
    }
}

fn view(editor: &text_editor::Content) -> Element<'_, Edit> {
    column![
        text("Pilot launch brief"),
        text_editor(editor).on_action(Edit),
    ]
    .padding(16)
    .into()
}

fn main() -> iced::Result {
    iced::application(
        || {
            (
                text_editor::Content::with_text("Keep the first release on the current schedule."),
                Task::none(),
            )
        },
        update,
        view,
    )
    .run()
}
