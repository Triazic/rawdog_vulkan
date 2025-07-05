use std::arch::breakpoint;

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
  let mut v = vec![0; 32];
  for _i in 1..=32 {
    if _i == 31 {
      dbg!("break");
    }
    let i = 32 - _i;
    let mask = (2 as u32).pow(i);
    let matches = n & mask == mask;
    if matches {
      v[(_i-1) as usize] = 1;
    }
  }
  v
}

// test for split_bits
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