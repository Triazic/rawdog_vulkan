#![allow(dead_code, unused_variables, unused_imports)]

use std::{ffi::CString, str::FromStr};
pub mod utils;
pub mod constants;
use itertools::Itertools;
use utils::{cstr};

fn main() {
  let entry = create_entry();
  let instance = create_instance(&entry);
  let (physical_device, device, memory_type_index, heap_index) = create_device(&instance);
  dbg!(heap_index);
  let heap_usage_1 = get_heap_usage(&instance, &physical_device);

  let buffer_size = (4096.0 * 1.9) as u64;
  let buffer = create_buffer(&device, buffer_size);
  {
    let heap_usage_2 = get_heap_usage(&instance, &physical_device);
    dbg!(heap_usage_2[1] - heap_usage_1[1]);
  }
  let memory_allocation = allocate_memory(&device, memory_type_index, buffer_size);
  {
    let heap_usage_2 = get_heap_usage(&instance, &physical_device);
    dbg!(heap_usage_2[1] - heap_usage_1[1]);
  }



  unsafe { device.device_wait_idle().expect("Failed to wait for device to become idle"); }
  unsafe { device.free_memory(memory_allocation, None); }
  unsafe { device.destroy_buffer(buffer, None); }
  unsafe { device.destroy_device(None); }
  unsafe { instance.destroy_instance(None); }
  println!("Finished");
}

fn create_entry() -> ash::Entry {
  let entry = unsafe { ash::Entry::load().expect("Could not load Vulkan") };
  entry
}

fn create_instance(entry: &ash::Entry) -> ash::Instance {
  // application info
  let application_name = cstr("My Application");
  let application_version = 1;
  let engine_name = cstr("My Engine");
  let engine_version = 1;
  
  let application_info = ash::vk::ApplicationInfo::default()
    .application_name(&application_name)
    .application_version(application_version)
    .engine_name(&engine_name)
    .engine_version(engine_version)
    .api_version(constants::API_VERSION);
  
  // check that required layers are supported
  let layers = unsafe { entry.enumerate_instance_layer_properties().expect("failed to enumerate instance layer properties") };
  let assert_layer_supported = |layer_name: &str| {
    let has = layers.iter().any(|layer| {
      let name = layer.layer_name_as_c_str().expect("could not get layer name").to_str().expect("could not convert layer name to &str");
      name == layer_name
    });
    assert!(has, "instance layer {} is not supported", layer_name);
  };
  constants::REQUIRED_INSTANCE_LAYERS.iter().for_each(|layer| assert_layer_supported(layer));

  // instance create info
  let flags = ash::vk::InstanceCreateFlags::empty();
  let layer_cstrs = constants::REQUIRED_INSTANCE_LAYERS.iter().map(|str| cstr(str)).collect_vec();
  let layer_ptrs: Vec<*const i8> = layer_cstrs.iter().map(|s| s.as_ptr()).collect();
  let extension_names = [];
  let instance_create_info = ash::vk::InstanceCreateInfo::default()
    .flags(flags)
    .application_info(&application_info)
    .enabled_layer_names(&layer_ptrs)
    .enabled_extension_names(&extension_names);

  // create instance
  let instance = unsafe { entry.create_instance(&instance_create_info, None).expect("Could not create Vulkan instance") };
  instance
}

fn create_device(instance: &ash::Instance) -> (ash::vk::PhysicalDevice, ash::Device, u32, u32) {
  // physical device
  let physical_devices = unsafe { instance.enumerate_physical_devices().expect("failed to enumerate physical devices") };
  // assert that there is at least one physical device
  assert!(physical_devices.len() > 0, "no physical devices found");
  // find the first suitable device
  let physical_device = *physical_devices.iter().find(|&physical_device| {
    let properties = unsafe { instance.get_physical_device_properties(*physical_device) };

    // number of memory allocations
    if properties.limits.max_memory_allocation_count < 1 { return false; }

    // memory flags
    let memory_properties = unsafe { instance.get_physical_device_memory_properties(*physical_device) };
    let memory_types = memory_properties.memory_types_as_slice();
    let has_suitable_memory_type = memory_types.iter().any(|memory_type| {
      get_if_memory_type_is_suitable(memory_type)
    });
    if !has_suitable_memory_type { return false; }

    // check vulkan version
    let required_vulkan_version = constants::API_VERSION;
    let supported_version = properties.api_version;
    if supported_version < required_vulkan_version { return false; }

    let limits = properties.limits;
    // dbg!(limits.buffer_image_granularity);
    // dbg!(limits.non_coherent_atom_size);

    let sparse_properties = properties.sparse_properties;

    let features = unsafe { instance.get_physical_device_features(*physical_device) };

    let device_memory_properties = unsafe { instance.get_physical_device_memory_properties(*physical_device) };

    let memory_types = device_memory_properties.memory_types_as_slice();

    // let physical_device_format_properties = unsafe { instance.get_physical_device_format_properties(*physical_device, format) };

    // let physical_device_image_properties = unsafe { instance.get_physical_device_image_format_properties(*physical_device, format) };

    // check for required device layers
    let layers = unsafe { instance.enumerate_device_layer_properties(*physical_device).expect("failed to enumerate device layer properties") };
    let layer_supported = |layer_name: &str| {
      layers.iter().any(|layer| {
        let name = layer.layer_name_as_c_str().expect("could not get layer name").to_str().expect("could not convert layer name to &str");
        name == layer_name
      })
    };
    let all_layers_supported = constants::REQUIRED_DEVICE_LAYERS.iter().all(|layer| layer_supported(layer));
    if !all_layers_supported { return false; }

    // check for required device extensions
    let extensions = unsafe { instance.enumerate_device_extension_properties(*physical_device).expect("failed to enumerate device extension properties") };
    let extension_supported = |extension_name: &str| {
      extensions.iter().any(|extension| {
        let name = extension.extension_name_as_c_str().expect("could not get extension name").to_str().expect("could not convert extension name to &str");
        name == extension_name
      })
    };
    let all_extensions_supported = constants::REQUIRED_DEVICE_EXTENSIONS.iter().all(|extension| extension_supported(extension));
    if !all_extensions_supported { return false; }

    true // device is adequate
  }).expect("no physical device satisifies the requirements of this application");

  let queue_family_properties = unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
  // find index of first suitable queue family
  assert!(queue_family_properties.len() > 0, "no queue families found");
  let queue_family_index = queue_family_properties.iter().position(|&properties| {
    if properties.queue_count < 1 { return false; }
    let req_flags = [ash::vk::QueueFlags::GRAPHICS, ash::vk::QueueFlags::COMPUTE, ash::vk::QueueFlags::TRANSFER];
    for req_flag in req_flags.into_iter() {
      if !properties.queue_flags.contains(req_flag) { return false; }
    }
    true // queue family is adequate
  }).expect("no queue family satisifies the requirements of this application");

  // queue create info
  let queue_priorities = [1.0];
  let main_queue =  ash::vk::DeviceQueueCreateInfo::default()
    .queue_family_index(queue_family_index as u32)
    .queue_priorities(&queue_priorities);

  let queue_create_infos = vec![main_queue];

  // device memory properties
  let memory_properties = unsafe { instance.get_physical_device_memory_properties(physical_device) };
  let memory_types = memory_properties.memory_types_as_slice();
  let memory_type_index = memory_types.iter().position(|memory_type| {
    return get_if_memory_type_is_suitable(memory_type);
  }).expect("failed to find suitable memory type") as u32;
  let memory_type = memory_types[memory_type_index as usize];
  let heap_index = memory_type.heap_index;

  // device create info
  let device_extensions = [];
  let device_features = ash::vk::PhysicalDeviceFeatures::default();
  let device_create_info = ash::vk::DeviceCreateInfo::default()
    .queue_create_infos(&queue_create_infos)
    .enabled_extension_names(&device_extensions)
    .enabled_features(&device_features);

  // create device
  let device = unsafe { instance.create_device(physical_device, &device_create_info, None).expect("Could not create Vulkan device") };
  (physical_device, device, memory_type_index, heap_index)
}

/// just a handle. not backed with memory
fn create_buffer(device: &ash::Device, buffer_size: u64) -> ash::vk::Buffer {
  let flags = ash::vk::BufferCreateFlags::empty();
  let usage = ash::vk::BufferUsageFlags::TRANSFER_SRC | ash::vk::BufferUsageFlags::TRANSFER_DST;
  let sharing_mode = ash::vk::SharingMode::EXCLUSIVE; // used in one queue
  let buffer_create_info = ash::vk::BufferCreateInfo::default()
    .flags(flags) 
    .size(buffer_size)
    .usage(usage)
    .sharing_mode(sharing_mode);

  let buffer = unsafe { device.create_buffer(&buffer_create_info, None).expect("Could not create Vulkan buffer") };
  buffer
}

fn create_image(device: &ash::Device) -> ash::vk::Image {
  let image_create_info = ash::vk::ImageCreateInfo::default();
  let image = unsafe { device.create_image(&image_create_info, None).expect("Could not create Vulkan image") };
  image
}

fn allocate_memory(device: &ash::Device, memory_type_index: u32, size: u64) -> ash::vk::DeviceMemory {
  let info = ash::vk::MemoryAllocateInfo::default()
    .allocation_size(size)
    .memory_type_index(memory_type_index);

  let memory = unsafe { device.allocate_memory(&info, None).expect("Could not allocate Vulkan memory") };
  memory
}

/// useless, only works with VK_MEMORY_PROPERTY_LAZILY_ALLOCATED_BIT which is basically never supported. (35.06%)
fn print_memory_commitment(device: &ash::Device, allocation: &ash::vk::DeviceMemory) -> () {
  let memory_commitment = unsafe { device.get_device_memory_commitment(*allocation) };
  dbg!(memory_commitment);
}

fn get_if_memory_type_is_suitable(memory_type: &ash::vk::MemoryType) -> bool {
  // no constraints yet
  return true;
}

fn get_heap_usage(instance: &ash::Instance, physical_device: &ash::vk::PhysicalDevice) -> [u64; ash::vk::MAX_MEMORY_HEAPS] {
  let mut memory_budget_props = ash::vk::PhysicalDeviceMemoryBudgetPropertiesEXT::default();
  
  let mut memory_props2 = ash::vk::PhysicalDeviceMemoryProperties2::default()
    .push_next(&mut memory_budget_props);
  
  unsafe { instance.get_physical_device_memory_properties2(*physical_device, &mut memory_props2); }
  
  let heap_usage = memory_budget_props.heap_usage;
  return heap_usage;
}

fn print_heap_usage(instance: &ash::Instance, physical_device: &ash::vk::PhysicalDevice) -> () {
  let heap_usage = get_heap_usage(instance, physical_device);
  dbg!(heap_usage);
}


