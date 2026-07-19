//! In-memory asset organizer combining stable multi-selection, typed drag and drop,
//! and routed commands.

use std::collections::HashSet;

use dear_app::{AppConfig, RunError, run_ui};
use dear_imgui_rs::{
    Condition, DragDropTargetFlags, Key, KeyChord, KeyMods, KeySetSelection, MultiSelectBoxSelect,
    MultiSelectFlags, MultiSelectOptions, MultiSelectScopeKind, ShortcutRoute, Ui, WindowFlags,
};

const ASSET_PAYLOAD: &str = "ASSET_ID";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct AssetId(u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Collection {
    Incoming,
    Review,
    Approved,
}

impl Collection {
    const ALL: [Self; 3] = [Self::Incoming, Self::Review, Self::Approved];

    const fn label(self) -> &'static str {
        match self {
            Self::Incoming => "Incoming",
            Self::Review => "In review",
            Self::Approved => "Approved",
        }
    }
}

struct Asset {
    id: AssetId,
    name: String,
    kind: &'static str,
    collection: Collection,
}

#[derive(Clone, Copy)]
enum Command {
    AddAsset,
    DeleteSelection,
    MoveAssets {
        dragged: AssetId,
        target: Collection,
    },
}

struct Organizer {
    assets: Vec<Asset>,
    selected: HashSet<AssetId>,
    current_collection: Collection,
    next_id: u32,
    status: String,
}

impl Default for Organizer {
    fn default() -> Self {
        let entries = [
            ("Landing page", "Design", Collection::Incoming),
            ("Brand icons", "Vector", Collection::Incoming),
            ("Product tour", "Video", Collection::Incoming),
            ("Release banner", "Design", Collection::Review),
            ("Pricing copy", "Document", Collection::Review),
            ("App icon", "Vector", Collection::Approved),
        ];
        let assets = entries
            .into_iter()
            .enumerate()
            .map(|(index, (name, kind, collection))| Asset {
                id: AssetId(index as u32 + 1),
                name: name.to_owned(),
                kind,
                collection,
            })
            .collect();

        Self {
            assets,
            selected: HashSet::new(),
            current_collection: Collection::Incoming,
            next_id: entries.len() as u32 + 1,
            status: "Ready".to_owned(),
        }
    }
}

impl Organizer {
    fn request(command: &mut Option<Command>, requested: bool, value: Command) {
        if requested {
            *command = Some(value);
        }
    }

    fn ui(&mut self, ui: &Ui) {
        let mut command = None;
        let has_selection = !self.selected.is_empty();
        let shortcut_route = ShortcutRoute::Focused;

        ui.window("Asset organizer")
            .size([860.0, 520.0], Condition::FirstUseEver)
            .flags(WindowFlags::MENU_BAR)
            .build(|| {
                if let Some(_menu_bar) = ui.begin_menu_bar() {
                    ui.menu("Asset", || {
                        Self::request(
                            &mut command,
                            ui.menu_item_with_shortcut("Add asset", "Ctrl+N"),
                            Command::AddAsset,
                        );
                        Self::request(
                            &mut command,
                            ui.menu_item_enabled_selected_with_shortcut(
                                "Delete selection",
                                "Delete",
                                false,
                                has_selection,
                            ),
                            Command::DeleteSelection,
                        );
                    });
                }

                Self::request(
                    &mut command,
                    ui.shortcut_with_flags(
                        KeyChord::new(Key::N).with_mods(KeyMods::CTRL),
                        shortcut_route,
                    ),
                    Command::AddAsset,
                );
                Self::request(
                    &mut command,
                    has_selection
                        && ui.shortcut_with_flags(KeyChord::new(Key::Delete), shortcut_route),
                    Command::DeleteSelection,
                );

                Self::request(&mut command, ui.button("Add asset"), Command::AddAsset);
                ui.same_line();
                {
                    let _disabled = ui.begin_disabled_with_cond(!has_selection);
                    Self::request(&mut command, ui.button("Delete"), Command::DeleteSelection);
                }
                ui.same_line();
                ui.text_disabled(format!(
                    "{} selected | {}",
                    self.selected.len(),
                    self.status
                ));
                ui.separator();

                ui.child_window("collections")
                    .size([190.0, 0.0])
                    .border(true)
                    .build(ui, || {
                        ui.text("Collections");
                        ui.separator();
                        for collection in Collection::ALL {
                            let count = self
                                .assets
                                .iter()
                                .filter(|asset| asset.collection == collection)
                                .count();
                            let label = format!(
                                "{} ({count})##collection-{collection:?}",
                                collection.label()
                            );
                            if ui
                                .selectable_config(label)
                                .selected(self.current_collection == collection)
                                .build()
                                && self.current_collection != collection
                            {
                                self.current_collection = collection;
                                self.selected.clear();
                            }

                            if let Some(target) = ui.drag_drop_target()
                                && let Some(Ok(payload)) = target.accept_payload::<AssetId, _>(
                                    ASSET_PAYLOAD,
                                    DragDropTargetFlags::NONE,
                                )
                                && payload.delivery
                                && collection != self.current_collection
                            {
                                command = Some(Command::MoveAssets {
                                    dragged: payload.data,
                                    target: collection,
                                });
                            }
                        }
                    });

                ui.same_line();
                ui.child_window("assets")
                    .size([0.0, 0.0])
                    .border(true)
                    .build(ui, || {
                        ui.text(self.current_collection.label());
                        ui.separator();

                        let indices = self
                            .assets
                            .iter()
                            .enumerate()
                            .filter_map(|(index, asset)| {
                                (asset.collection == self.current_collection).then_some(index)
                            })
                            .collect::<Vec<_>>();
                        let keys = indices
                            .iter()
                            .map(|&index| self.assets[index].id)
                            .collect::<Vec<_>>();
                        let assets = &self.assets;
                        let mut selection = KeySetSelection::new(&keys, &mut self.selected);
                        let options = MultiSelectOptions::new()
                            .flags(
                                MultiSelectFlags::CLEAR_ON_ESCAPE
                                    | MultiSelectFlags::CLEAR_ON_CLICK_VOID,
                            )
                            .box_select(MultiSelectBoxSelect::OneDimensional)
                            .scope(MultiSelectScopeKind::Rect);

                        ui.multi_select_indexed(
                            &mut selection,
                            options,
                            |ui, visible_index, is_selected| {
                                let asset = &assets[indices[visible_index]];
                                let label = format!(
                                    "{}  [{}]##asset-{}",
                                    asset.name, asset.kind, asset.id.0
                                );
                                ui.selectable_config(label).selected(is_selected).build();

                                if let Some(_source) = ui
                                    .drag_drop_source_config(ASSET_PAYLOAD)
                                    .begin_payload(asset.id)
                                {
                                    ui.text(&asset.name);
                                    ui.text_disabled(asset.kind);
                                }
                            },
                        );
                    });
            });

        if let Some(command) = command {
            self.execute(command);
        }
    }

    fn execute(&mut self, command: Command) {
        match command {
            Command::AddAsset => {
                let id = AssetId(self.next_id);
                self.next_id += 1;
                self.assets.push(Asset {
                    id,
                    name: format!("Untitled asset {}", id.0),
                    kind: "Draft",
                    collection: self.current_collection,
                });
                self.selected.clear();
                self.selected.insert(id);
                self.status = "Asset added".to_owned();
            }
            Command::DeleteSelection => {
                self.assets
                    .retain(|asset| !self.selected.contains(&asset.id));
                self.selected.clear();
                self.status = "Selection deleted".to_owned();
            }
            Command::MoveAssets { dragged, target } => {
                if !self.selected.contains(&dragged) {
                    self.selected.clear();
                    self.selected.insert(dragged);
                }
                let selected = &self.selected;
                for asset in &mut self.assets {
                    if selected.contains(&asset.id) {
                        asset.collection = target;
                    }
                }
                self.current_collection = target;
                self.status = format!("Moved to {}", target.label());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collection_of(organizer: &Organizer, id: AssetId) -> Collection {
        organizer
            .assets
            .iter()
            .find(|asset| asset.id == id)
            .expect("test asset should exist")
            .collection
    }

    #[test]
    fn add_asset_selects_it_in_the_current_collection() {
        let mut organizer = Organizer::default();
        organizer.current_collection = Collection::Review;

        organizer.execute(Command::AddAsset);

        let added = organizer.assets.last().expect("asset should be added");
        assert_eq!(added.id, AssetId(7));
        assert_eq!(added.name, "Untitled asset 7");
        assert_eq!(added.collection, Collection::Review);
        assert_eq!(organizer.selected, HashSet::from([added.id]));
        assert_eq!(organizer.next_id, 8);
    }

    #[test]
    fn delete_selection_removes_every_selected_asset() {
        let mut organizer = Organizer::default();
        organizer.selected = HashSet::from([AssetId(1), AssetId(4)]);

        organizer.execute(Command::DeleteSelection);

        assert_eq!(organizer.assets.len(), 4);
        assert!(
            organizer
                .assets
                .iter()
                .all(|asset| asset.id != AssetId(1) && asset.id != AssetId(4))
        );
        assert!(organizer.selected.is_empty());
    }

    #[test]
    fn move_assets_moves_the_existing_selection_as_a_group() {
        let mut organizer = Organizer::default();
        organizer.selected = HashSet::from([AssetId(1), AssetId(2)]);

        organizer.execute(Command::MoveAssets {
            dragged: AssetId(1),
            target: Collection::Approved,
        });

        assert_eq!(collection_of(&organizer, AssetId(1)), Collection::Approved);
        assert_eq!(collection_of(&organizer, AssetId(2)), Collection::Approved);
        assert_eq!(collection_of(&organizer, AssetId(3)), Collection::Incoming);
        assert_eq!(organizer.current_collection, Collection::Approved);
        assert_eq!(organizer.selected, HashSet::from([AssetId(1), AssetId(2)]));
    }

    #[test]
    fn dragging_an_unselected_asset_moves_only_that_asset() {
        let mut organizer = Organizer::default();
        organizer.selected = HashSet::from([AssetId(1)]);

        organizer.execute(Command::MoveAssets {
            dragged: AssetId(2),
            target: Collection::Review,
        });

        assert_eq!(collection_of(&organizer, AssetId(1)), Collection::Incoming);
        assert_eq!(collection_of(&organizer, AssetId(2)), Collection::Review);
        assert_eq!(organizer.selected, HashSet::from([AssetId(2)]));
        assert_eq!(organizer.current_collection, Collection::Review);
    }
}

fn main() -> Result<(), RunError> {
    let config = AppConfig {
        window_title: "Dear ImGui - Asset Organizer".to_owned(),
        window_size: (1000.0, 680.0),
        ..Default::default()
    };
    let mut organizer = Organizer::default();

    run_ui(config, move |ui| organizer.ui(ui))
}
