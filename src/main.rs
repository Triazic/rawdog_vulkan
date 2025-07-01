#![allow(dead_code, unused_variables, unused_imports)]

use std::{ffi::CString, str::FromStr};
pub mod utils;
use utils::{cstr};

fn main() {
  let entry = create_entry();
  let instance = create_instance(&entry);
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
  let api_version = ash::vk::API_VERSION_1_0;
  let application_info = ash::vk::ApplicationInfo::default()
    .application_name(&application_name)
    .application_version(application_version)
    .engine_name(&engine_name)
    .engine_version(engine_version)
    .api_version(api_version);

  // instance create info
  let flags = ash::vk::InstanceCreateFlags::empty();
  let layer_names = [];
  let extension_names = [];
  let instance_create_info = ash::vk::InstanceCreateInfo::default()
    .flags(flags)
    .application_info(&application_info)
    .enabled_layer_names(&layer_names)
    .enabled_extension_names(&extension_names);

  // create instance
  let instance = unsafe { entry.create_instance(&instance_create_info, None).expect("Could not create Vulkan instance") };
  instance
}


