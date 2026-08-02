//! Checked-in SPIR-V generated from the adjacent GLSL sources.

use std::io::{self, Cursor};

const VERT_SPV: &[u8] = include_bytes!("../shaders/shader.vert.spv");
const FRAG_SPV: &[u8] = include_bytes!("../shaders/shader.frag.spv");

pub(super) fn vertex_spirv() -> io::Result<Vec<u32>> {
    ash::util::read_spv(&mut Cursor::new(VERT_SPV))
}

pub(super) fn fragment_spirv() -> io::Result<Vec<u32>> {
    ash::util::read_spv(&mut Cursor::new(FRAG_SPV))
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::*;

    const OP_TYPE_IMAGE: u16 = 25;
    const OP_TYPE_SAMPLER: u16 = 26;
    const OP_TYPE_POINTER: u16 = 32;
    const OP_VARIABLE: u16 = 59;
    const OP_DECORATE: u16 = 71;
    const DECORATION_BINDING: u32 = 33;
    const DECORATION_DESCRIPTOR_SET: u32 = 34;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum DescriptorKind {
        Image,
        Sampler,
    }

    fn reflected_descriptors(words: &[u32]) -> Vec<(u32, u32, DescriptorKind)> {
        let mut image_types = HashSet::new();
        let mut sampler_types = HashSet::new();
        let mut pointer_targets = HashMap::new();
        let mut variables = HashMap::new();
        let mut sets = HashMap::new();
        let mut bindings = HashMap::new();
        let mut cursor = 5;

        while cursor < words.len() {
            let header = words[cursor];
            let word_count = (header >> 16) as usize;
            let opcode = header as u16;
            assert!(word_count > 0, "SPIR-V instruction has zero words");
            assert!(
                cursor + word_count <= words.len(),
                "SPIR-V instruction extends past the module"
            );
            let operands = &words[cursor + 1..cursor + word_count];
            match opcode {
                OP_TYPE_IMAGE => {
                    image_types.insert(operands[0]);
                }
                OP_TYPE_SAMPLER => {
                    sampler_types.insert(operands[0]);
                }
                OP_TYPE_POINTER => {
                    pointer_targets.insert(operands[0], operands[2]);
                }
                OP_VARIABLE => {
                    variables.insert(operands[1], operands[0]);
                }
                OP_DECORATE if operands.len() >= 3 => match operands[1] {
                    DECORATION_BINDING => {
                        bindings.insert(operands[0], operands[2]);
                    }
                    DECORATION_DESCRIPTOR_SET => {
                        sets.insert(operands[0], operands[2]);
                    }
                    _ => {}
                },
                _ => {}
            }
            cursor += word_count;
        }

        let mut descriptors = variables
            .into_iter()
            .filter_map(|(variable, pointer)| {
                let pointee = *pointer_targets.get(&pointer)?;
                let kind = if image_types.contains(&pointee) {
                    DescriptorKind::Image
                } else if sampler_types.contains(&pointee) {
                    DescriptorKind::Sampler
                } else {
                    return None;
                };
                Some((*sets.get(&variable)?, *bindings.get(&variable)?, kind))
            })
            .collect::<Vec<_>>();
        descriptors.sort_unstable_by_key(|(set, binding, _)| (*set, *binding));
        descriptors
    }

    #[test]
    fn fragment_source_and_spirv_use_split_image_and_sampler_sets() {
        let source = include_str!("../shaders/shader.frag");
        assert!(source.contains("set = 0) uniform texture2D"));
        assert!(source.contains("set = 1) uniform sampler"));

        let words = fragment_spirv().expect("checked-in fragment SPIR-V must parse");
        assert_eq!(
            reflected_descriptors(&words),
            vec![
                (0, 0, DescriptorKind::Image),
                (1, 0, DescriptorKind::Sampler),
            ]
        );
    }

    #[test]
    fn checked_in_vertex_spirv_parses() {
        vertex_spirv().expect("checked-in vertex SPIR-V must parse");
    }
}
