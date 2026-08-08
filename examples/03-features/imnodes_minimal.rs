//! Minimal ImNodes integration with the high-level `dear-app` runtime.

use dear_app::{
    AddOnsConfig, AppConfig, Application, ApplicationStage, FrameContext, RunError,
    ShutdownContext, run,
};
use dear_imgui_rs::Condition;
use dear_imnodes::{EditorContext, ImNodesExt, LinkId, NodeId, PinId, PinShape};

const SOURCE_NODE: NodeId = NodeId::new(1);
const SOURCE_OUTPUT: PinId = PinId::new(11);
const TARGET_NODE: NodeId = NodeId::new(2);
const TARGET_INPUT: PinId = PinId::new(21);
const LINK: LinkId = LinkId::new(100);

#[derive(Default)]
struct ImNodesMinimal {
    editor: Option<EditorContext>,
    layout_initialized: bool,
}

impl Application for ImNodesMinimal {
    fn frame(&mut self, context: &mut FrameContext<'_>) -> Result<(), RunError> {
        let nodes_context = context.addons().imnodes();
        let ui = context.ui();
        let Some(nodes_context) = nodes_context else {
            ui.text("ImNodes add-on not enabled");
            return Ok(());
        };
        if self.editor.is_none() {
            let editor = nodes_context
                .try_create_editor_context()
                .map_err(|error| RunError::application(ApplicationStage::Frame, error))?;
            self.editor = Some(editor);
        }
        let editor = self.editor.as_ref().ok_or_else(|| {
            RunError::application(
                ApplicationStage::Frame,
                std::io::Error::other("the ImNodes editor was not initialized"),
            )
        })?;
        let initialize_layout = !self.layout_initialized;
        let mut submitted = false;

        ui.window("ImNodes Minimal")
            .size([720.0, 520.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Two nodes connected by one persistent link.");
                ui.separator();

                let nodes = ui.imnodes(nodes_context);
                let setup = nodes.editor(Some(editor));
                if initialize_layout {
                    setup.set_node_pos_grid(SOURCE_NODE, [40.0, 80.0]);
                    setup.set_node_pos_grid(TARGET_NODE, [340.0, 120.0]);
                }

                let editor = setup.begin_nodes();
                {
                    let node = editor.node(SOURCE_NODE);
                    node.title_bar(|| ui.text("Source"));
                    {
                        let _output = node.output_attr(SOURCE_OUTPUT, PinShape::CircleFilled);
                        ui.text("value");
                    }
                }
                {
                    let node = editor.node(TARGET_NODE);
                    node.title_bar(|| ui.text("Target"));
                    {
                        let _input = node.input_attr(TARGET_INPUT, PinShape::CircleFilled);
                        ui.text("value");
                    }
                }
                editor.link(LINK, SOURCE_OUTPUT, TARGET_INPUT);
                let _post = editor.end();
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
        window_title: "Dear ImGui - ImNodes Minimal".to_owned(),
        window_size: (900.0, 680.0),
        addons: AddOnsConfig {
            with_imnodes: true,
            ..Default::default()
        },
        ..Default::default()
    };

    run(config, ImNodesMinimal::default())
}
