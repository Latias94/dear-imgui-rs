//! Minimal imgui-node-editor integration with the high-level `dear-app` runtime.

use dear_app::{
    AppConfig, Application, ApplicationStage, FrameContext, InitContext, RunError, ShutdownContext,
    run,
};
use dear_imgui_rs::Condition;
use dear_node_editor::{
    EditorConfig, EditorContext, LinkId, NodeEditorUiExt, NodeId, PinId, PinKind,
};

const SOURCE_NODE: NodeId = NodeId::new(1);
const SOURCE_OUTPUT: PinId = PinId::new(11);
const TARGET_NODE: NodeId = NodeId::new(2);
const TARGET_INPUT: PinId = PinId::new(21);
const LINK: LinkId = LinkId::new(100);

#[derive(Default)]
struct NodeEditorMinimal {
    editor: Option<EditorContext>,
    layout_initialized: bool,
}

impl Application for NodeEditorMinimal {
    fn configure_imgui(&mut self, context: &mut InitContext<'_>) -> Result<(), RunError> {
        let editor =
            EditorContext::try_create_with_config(context.imgui(), EditorConfig::default())
                .map_err(|error| RunError::application(ApplicationStage::ConfigureImgui, error))?;
        self.editor = Some(editor);
        Ok(())
    }

    fn frame(&mut self, context: &mut FrameContext<'_>) -> Result<(), RunError> {
        let editor_context = self.editor.as_ref().ok_or_else(|| {
            RunError::application(
                ApplicationStage::Frame,
                std::io::Error::other("the node editor was not initialized"),
            )
        })?;
        let initialize_layout = !self.layout_initialized;
        let mut submitted = false;
        let ui = context.ui();

        ui.window("Node Editor Minimal")
            .size([720.0, 520.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Two nodes connected by one persistent link.");
                ui.separator();

                let available = ui.content_region_avail();
                let editor = ui.node_editor(
                    editor_context,
                    "node_editor_minimal",
                    [available[0].max(1.0), available[1].max(1.0)],
                );
                if initialize_layout {
                    editor.set_node_position(SOURCE_NODE, [40.0, 80.0]);
                    editor.set_node_position(TARGET_NODE, [340.0, 120.0]);
                }

                editor.node(SOURCE_NODE, |node| {
                    ui.text("Source");
                    node.pin(SOURCE_OUTPUT, PinKind::Output, |_| ui.text("value"));
                });
                editor.node(TARGET_NODE, |node| {
                    ui.text("Target");
                    node.pin(TARGET_INPUT, PinKind::Input, |_| ui.text("value"));
                });
                editor.link(LINK, SOURCE_OUTPUT, TARGET_INPUT);
                if initialize_layout {
                    editor.navigate_to_content(0.0);
                }
                editor.end();
                submitted = true;
            });

        self.layout_initialized |= submitted;
        Ok(())
    }

    fn shutdown(&mut self, _context: &mut ShutdownContext<'_>) -> Result<(), RunError> {
        self.editor = None;
        Ok(())
    }
}

fn main() -> Result<(), RunError> {
    let config = AppConfig {
        window_title: "Dear ImGui - Node Editor Minimal".to_owned(),
        window_size: (900.0, 680.0),
        ..Default::default()
    };

    run(config, NodeEditorMinimal::default())
}
