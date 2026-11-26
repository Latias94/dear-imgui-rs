use dear_imgui_rs::{Context, FramePrepareOptions};
use dear_imnodes::{ImNodesContext, ImNodesExt, LinkId, NodeId, PinId, PinShape};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn prepare_imgui(imgui: &mut Context) {
    imgui.prepare_frame(FramePrepareOptions::new([800.0, 600.0], 1.0 / 60.0));
    imgui
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("headless test requires the legacy font-atlas capability")
        .build();
    let _ = imgui.set_ini_filename::<std::path::PathBuf>(None);
}

#[test]
fn title_only_nodes_include_the_body_origin_in_the_group_bounds() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let nodes = ImNodesContext::create(&imgui);
    let ui = imgui.frame();
    ui.window("Node bounds")
        .size([400.0, 300.0], dear_imgui_rs::Condition::Always)
        .build(|| {
            ui.imnodes(&nodes).editor(None).with_nodes(|editor| {
                let body_origin = editor.node(NodeId::new(1), |node| {
                    node.title_bar(|| ui.text("Title"));
                    ui.cursor_screen_pos()
                });
                assert!(ui.item_rect_max()[1] >= body_origin[1]);

                let body_bottom = editor.node(NodeId::new(2), |node| {
                    node.title_bar(|| ui.text("Title"));
                    ui.dummy([100.0, 30.0]);
                    ui.item_rect_max()[1]
                });
                assert_eq!(ui.item_rect_max()[1], body_bottom);
            });
        })
        .unwrap();
    assert!(imgui.end_frame());
}

#[test]
fn each_node_accepts_only_one_title_bar() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let nodes = ImNodesContext::create(&imgui);
    let ui = imgui.frame();
    ui.imnodes(&nodes).editor(None).with_nodes(|editor| {
        editor.node(NodeId::new(1), |node| {
            node.title_bar(|| {
                ui.text("First title");
                assert!(catch_unwind(AssertUnwindSafe(|| node.title_bar(|| {}))).is_err());
            });
            assert!(catch_unwind(AssertUnwindSafe(|| node.title_bar(|| {}))).is_err());
        });
        editor.node(NodeId::new(2), |node| {
            node.title_bar(|| ui.text("Another node"))
        });
    });
    assert!(imgui.end_frame());
}

#[test]
fn queued_unsubmitted_node_options_do_not_touch_native_node_bookkeeping() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let nodes = ImNodesContext::create(&imgui);
    let editor_context = nodes.create_editor_context();

    let ui = imgui.frame();
    ui.window("queued options")
        .size([400.0, 300.0], dear_imgui_rs::Condition::Always)
        .build(|| {
            let setup = ui.imnodes(&nodes).editor(Some(&editor_context));
            setup.set_node_pos_grid(NodeId::new(900), [120.0, 80.0]);
            setup.set_node_draggable(NodeId::new(900), false);
            setup.snap_node_to_grid(NodeId::new(900));

            // No native setter is invoked because node 900 is never submitted.
            assert!(
                !setup
                    .end()
                    .save_state_to_ini_string()
                    .contains("[node.900]")
            );
        });
    assert!(imgui.end_frame());
}

#[test]
fn selection_tracks_submitted_nodes_and_links() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let nodes = ImNodesContext::create(&imgui);
    let graph = nodes.create_editor_context();
    let keep = NodeId::new(1);
    let remove = NodeId::new(2);
    let output = PinId::new(10);
    let input = PinId::new(20);
    let link = LinkId::new(30);

    let ui = imgui.frame();
    let first = ui
        .window("Initial graph")
        .size([400.0, 300.0], dear_imgui_rs::Condition::Always)
        .build(|| {
            ui.imnodes(&nodes)
                .editor(Some(&graph))
                .with_nodes(|editor| {
                    editor.node(keep, |node| {
                        node.output_attr(output, PinShape::CircleFilled, || ui.text("out"));
                    });
                    editor.node(remove, |node| {
                        node.input_attr(input, PinShape::CircleFilled, || ui.text("in"));
                    });
                    editor.link(link, output, input);
                })
        })
        .unwrap();

    for _ in 0..2 {
        assert!(first.select_node(remove));
        assert!(first.select_link(link));
    }
    assert_eq!(first.selected_nodes(), [remove]);
    assert_eq!(first.selected_links(), [link]);
    for _ in 0..2 {
        assert!(first.clear_node_selection_of(remove));
        assert!(first.clear_link_selection_of(link));
    }
    assert!(first.selected_nodes().is_empty());
    assert!(first.selected_links().is_empty());
    assert!(first.select_node(keep));
    assert!(first.select_node(remove));
    assert!(first.select_link(link));

    ui.window("Independent graph")
        .size([400.0, 300.0], dear_imgui_rs::Condition::Always)
        .build(|| {
            ui.imnodes(&nodes).editor(None).end();
        })
        .unwrap();
    assert!(first.is_node_selected(remove));
    assert!(first.is_link_selected(link));

    ui.window("Updated graph")
        .size([400.0, 300.0], dear_imgui_rs::Condition::Always)
        .build(|| {
            ui.imnodes(&nodes)
                .editor(Some(&graph))
                .with_nodes(|editor| {
                    editor.node(keep, |_| {});
                });
        })
        .unwrap();
    assert!(!first.select_node(remove));
    assert!(!first.select_link(link));
    assert!(!first.center_on_submitted_node(remove));
    assert_eq!(first.selected_nodes(), [keep]);
    assert!(first.selected_links().is_empty());
    drop(first);
    assert!(imgui.end_frame());
}

#[test]
fn editor_scopes_own_their_explicit_editor_context_lease() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let nodes = ImNodesContext::create(&imgui);

    let editor_context = nodes.create_editor_context();
    let bound = nodes.bind_editor(&editor_context);
    drop(editor_context);
    assert_eq!(bound.get_panning(), [0.0, 0.0]);
    drop(bound);

    let editor_context = nodes.create_editor_context();
    let ui = imgui.frame();
    ui.window("owned editor lease")
        .size([400.0, 300.0], dear_imgui_rs::Condition::Always)
        .build(|| {
            let setup = ui.imnodes(&nodes).editor(Some(&editor_context));
            drop(editor_context);

            let post = setup.with_nodes(|editor| editor.node(NodeId::new(1), |_| {}));
            assert!(post.select_node(NodeId::new(1)));
        });
    assert!(imgui.end_frame());
}

#[test]
fn unwinding_an_attribute_restores_the_host_and_allows_another_editor() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let nodes = ImNodesContext::create(&imgui);
    let ui = imgui.frame();
    ui.window("Parent")
        .size([400.0, 300.0], dear_imgui_rs::Condition::Always)
        .build(|| {
            let window = || unsafe { dear_imgui_rs::sys::igGetCurrentWindowRead() };
            let parent = window();
            let parent_ids = unsafe { (*parent).IDStack.Size };
            ui.open_popup("Graph");
            let popup = ui.begin_popup("Graph").unwrap();
            let popup_window = window();
            let mut tooltip = None;
            let result = catch_unwind(AssertUnwindSafe(|| {
                ui.imnodes(&nodes).editor(None).with_nodes(|editor| {
                    assert_ne!(window(), popup_window);
                    drop(popup);
                    editor.node(NodeId::new(1), |node| {
                        node.title_bar(|| ui.text("Title"));
                        let id = ui.push_id("outside pin");
                        node.input_attr(PinId::new(11), PinShape::CircleFilled, || {
                            drop(id);
                            tooltip = ui.begin_tooltip();
                            panic!("pin contents");
                        });
                    });
                });
            }));
            assert_eq!(
                result.unwrap_err().downcast_ref::<&str>(),
                Some(&"pin contents")
            );
            assert!(tooltip.is_some());
            assert_eq!(window(), parent);
            drop(tooltip);
            assert_eq!(unsafe { (*parent).IDStack.Size }, parent_ids);
            let _table = ui.begin_table("Graphs", 2).unwrap();
            ui.table_next_column();
            let column = ui.table_get_column_index().unwrap();
            let post = ui.imnodes(&nodes).editor(None).with_nodes(|editor| {
                editor.node(NodeId::new(2), |node| {
                    node.title_bar(|| ui.text("Recovered"))
                });
            });
            assert!(post.select_node(NodeId::new(2)));
            assert_eq!(ui.table_get_column_index(), Some(column));
            ui.table_next_column();
            ui.text("Next cell");
        })
        .unwrap();
    assert!(imgui.end_frame());
}

#[test]
fn submissions_are_bound_to_their_canvas_node_and_connectable_pins() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let nodes = ImNodesContext::create(&imgui);
    let other = ImNodesContext::create(&imgui);
    let explicit = nodes.create_editor_context();
    let ui = imgui.frame();
    ui.window("Outer graph")
        .size([400.0, 300.0], dear_imgui_rs::Condition::Always)
        .build(|| {
            let next = ui.imnodes(&nodes).editor(Some(&explicit));
            let post = ui.imnodes(&nodes).editor(None).with_nodes(|editor| {
                assert!(catch_unwind(AssertUnwindSafe(|| next.set_grid_spacing(32.0))).is_err());
                assert!(catch_unwind(AssertUnwindSafe(|| next.end())).is_err());
                let a = NodeId::new(1);
                let b = NodeId::new(2);
                let output = PinId::new(10);
                let input = PinId::new(20);
                let label = PinId::new(21);
                ui.window("Inner graph")
                    .size([240.0, 200.0], dear_imgui_rs::Condition::Always)
                    .build(|| {
                        ui.imnodes(&other).editor(None).with_nodes(|inner| {
                            assert!(
                                catch_unwind(AssertUnwindSafe(|| editor.node(a, |_| {}))).is_err()
                            );
                            inner.node(a, |node| node.title_bar(|| ui.text("Inner")));
                        });
                    })
                    .unwrap();
                editor.node(a, |node| {
                    node.output_attr(output, PinShape::CircleFilled, || {
                        assert!(
                            catch_unwind(AssertUnwindSafe(|| node.static_attr(label, || {})))
                                .is_err()
                        );
                    });
                });
                editor.node(b, |node| {
                    node.static_attr(label, || ui.text("Label"));
                    node.input_attr(input, PinShape::CircleFilled, || ui.text("In"));
                    assert!(
                        catch_unwind(AssertUnwindSafe(|| node.static_attr(input, || {}))).is_err()
                    );
                });
                let link = LinkId::new(30);
                assert!(
                    catch_unwind(AssertUnwindSafe(|| editor.link(link, label, input))).is_err()
                );
                editor.link(link, output, input);
                assert!(
                    catch_unwind(AssertUnwindSafe(
                        || editor.minimap(0.0, dear_imnodes::MiniMapLocation::TopRight)
                    ))
                    .is_err()
                );
                editor.minimap(0.25, dear_imnodes::MiniMapLocation::TopRight);
                assert!(
                    catch_unwind(AssertUnwindSafe(|| editor.node(NodeId::new(3), |_| {}))).is_err()
                );
            });
            assert!(post.select_link(LinkId::new(30)));
            ui.imnodes(&nodes).editor(Some(&explicit)).end();
        })
        .unwrap();
    assert!(imgui.end_frame());
}

#[test]
fn temporary_style_stacks_restore_configuration_for_the_next_editor() {
    use dear_imnodes::{ColorElement, StyleVar};
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let nodes = ImNodesContext::create(&imgui);
    let ui = imgui.frame();
    let setup = ui.imnodes(&nodes).editor(None);
    let color = ColorElement::TitleBar;
    let original = setup.get_color(color);
    setup.with_nodes(|editor| {
        let first = editor.push_color(color, [1.0; 4]);
        let second = editor.push_color(color, [0.0; 4]);
        first.pop();
        std::mem::forget(second);
        assert!(
            catch_unwind(AssertUnwindSafe(
                || editor.push_style_var_f32(StyleVar::NodePadding, 3.0)
            ))
            .is_err()
        );
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                editor.push_style_var_f32(StyleVar::LinkLineSegmentsPerLength, f32::NAN)
            }))
            .is_err()
        );
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                editor.push_style_var_vec2(StyleVar::MiniMapPadding, [4.0, f32::INFINITY])
            }))
            .is_err()
        );
        let _padding = editor.push_style_var_vec2(StyleVar::NodePadding, [3.0; 2]);
    });
    assert_eq!(ui.imnodes(&nodes).editor(None).get_color(color), original);
    assert!(imgui.end_frame());
}

#[test]
fn editor_rejects_invalid_panning_and_layout_before_native_mutation() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let nodes = ImNodesContext::create(&imgui);
    let editor = nodes.create_editor_context();
    let bound = nodes.bind_editor(&editor);
    bound.reset_panning([-7.0, 12.0]);
    assert!(catch_unwind(AssertUnwindSafe(|| bound.reset_panning([f32::NAN, 2.0]))).is_err());
    assert_eq!(bound.get_panning(), [-7.0, 12.0]);

    let ui = imgui.frame();
    let setup = ui.imnodes(&nodes).editor(Some(&editor));
    assert!(catch_unwind(AssertUnwindSafe(|| setup.set_auto_panning_speed(f32::NAN))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| setup.set_node_padding([f32::NAN, 0.0]))).is_err());
    assert!(
        catch_unwind(AssertUnwindSafe(
            || setup.set_minimap_padding([0.0, f32::INFINITY])
        ))
        .is_err()
    );
    assert!(
        catch_unwind(AssertUnwindSafe(
            || setup.set_minimap_offset([0.0, f32::INFINITY])
        ))
        .is_err()
    );
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            setup.set_link_line_segments_per_length(f32::INFINITY)
        }))
        .is_err()
    );
    setup.set_auto_panning_speed(200.0);
    setup.set_node_padding([4.0, 8.0]);
    setup.set_minimap_padding([4.0, 8.0]);
    setup.set_minimap_offset([-2.0, 2.0]);
    setup.set_link_line_segments_per_length(0.1);
    setup.end();
    assert!(imgui.end_frame());
}
