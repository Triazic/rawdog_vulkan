use itertools::Itertools;
use strum::IntoEnumIterator;
use crate::constants::{self, MemoryKind};

pub fn get_memory_type_index(instance: &ash::Instance, physical_device: &ash::vk::PhysicalDevice, required_flags: &[ash::vk::MemoryPropertyFlags]) -> Option<u32> {
  let memory_properties = unsafe { instance.get_physical_device_memory_properties(*physical_device) };
  let memory_types = memory_properties.memory_types_as_slice();
  let memory_type_index = memory_types.iter().position(|memory_type| {
    let flags = memory_type.property_flags;
    required_flags.iter().all(|required_flag| flags.contains(*required_flag))
  });
  return memory_type_index.map(|v| v as u32);
}

fn split_bits(n: u32) -> Vec<u32> {
  (0..32).rev()
    .map(|i| {
      let mask = 1 << i;
      let matches = n & mask == mask;
      if matches {
          1
      } else {
          0
      }
    })
    .collect()
}

fn split_flags(flags: ash::vk::MemoryPropertyFlags) -> Vec<ash::vk::MemoryPropertyFlags> {
  let bits = split_bits(flags.as_raw());
  let values = bits.iter()
    .enumerate().map(|(i, b)| if *b == 1 { 2u32.pow((32 - i - 1) as u32) } else { 0 })
    .filter(|v| *v != 0)
    .collect_vec();
  let flags = values.iter().map(|v| ash::vk::MemoryPropertyFlags::from_raw(*v)).collect_vec();
  flags
}

#[test]
fn test_split_bits() {
  let n = 0b10101010;
  let res = split_bits(n);
  let mut expected = vec![0; 32];
  expected[31] = 0;
  expected[30] = 1;
  expected[29] = 0;
  expected[28] = 1;
  expected[27] = 0;
  expected[26] = 1;
  expected[25] = 0;
  expected[24] = 1;
  assert_eq!(res, expected);
}

#[test]
fn test_split_flags() {
  let flags = ash::vk::MemoryPropertyFlags::DEVICE_LOCAL | ash::vk::MemoryPropertyFlags::HOST_VISIBLE;
  let res = split_flags(flags);
  assert!(res.len() == 2);
  assert!(res.contains(&ash::vk::MemoryPropertyFlags::DEVICE_LOCAL));
  assert!(res.contains(&ash::vk::MemoryPropertyFlags::HOST_VISIBLE));
}

pub fn get_memory_type_index_raw(instance: &ash::Instance, physical_device: &ash::vk::PhysicalDevice, required_flags: u32) -> Option<u32> {
  let memory_properties = unsafe { instance.get_physical_device_memory_properties(*physical_device) };
  let memory_types = memory_properties.memory_types_as_slice();
  let required_flags = ash::vk::MemoryPropertyFlags::from_raw(required_flags);
  // let split = split_bits(required_flags.as_raw());
  // let x = 4;
  let memory_type_index = memory_types.iter().position(|memory_type| {
    let flags = memory_type.property_flags;
    required_flags & flags == required_flags
  });
  return memory_type_index.map(|v| v as u32);
}

pub fn get_heap_index(instance: &ash::Instance, physical_device: &ash::vk::PhysicalDevice, index: u32) -> u32 {
  let memory_properties = unsafe { instance.get_physical_device_memory_properties(*physical_device) };
  let memory_type = memory_properties.memory_types_as_slice().get(index as usize).expect("memory type index out of bounds?");
  let heap_index = memory_type.heap_index;
  heap_index
}

pub fn get_if_physical_device_supports_all_memory_requirements(instance: &ash::Instance, physical_device: &ash::vk::PhysicalDevice) -> bool {
  for kind in MemoryKind::iter() {
    let flags = get_memory_flags_from_kind(kind);
    let memory_type_index = get_memory_type_index(&instance, &physical_device, &flags);
    if memory_type_index.is_none() { return false; }
  }
  return true;
}

pub fn get_memory_flags_from_kind(kind: MemoryKind) -> Vec<ash::vk::MemoryPropertyFlags> {
  match kind {
    MemoryKind::Buffer1 => vec![
      ash::vk::MemoryPropertyFlags::HOST_VISIBLE,
    ],
  }
}

pub fn get_memory_flags_raw(flags: &[ash::vk::MemoryPropertyFlags]) -> u32 {
  let mut raw_flags = 0;
  for flag in flags.iter() {
    raw_flags |= flag.as_raw();
  }
  return raw_flags;
}

pub fn get_memory_type_from_index(instance: &ash::Instance, physical_device: &ash::vk::PhysicalDevice, index: u32) -> ash::vk::MemoryType {
  let memory_properties = unsafe { instance.get_physical_device_memory_properties(*physical_device) };
  let memory_types = memory_properties.memory_types_as_slice();
  let memory_type = memory_types.get(index as usize).expect("memory type index out of bounds?");
  return *memory_type;
}

pub fn get_memory_type_flags_from_index(instance: &ash::Instance, physical_device: &ash::vk::PhysicalDevice, index: u32) -> ash::vk::MemoryPropertyFlags {
  let memory_type = get_memory_type_from_index(instance, physical_device, index);
  let flags = memory_type.property_flags;
  return flags;
}