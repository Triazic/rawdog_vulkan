pub static API_VERSION: u32 = ash::vk::API_VERSION_1_1;

pub static REQUIRED_INSTANCE_LAYERS: [&str; 1] = ["VK_LAYER_KHRONOS_validation"];

pub static REQUIRED_DEVICE_LAYERS: [&str; 1] = ["VK_LAYER_KHRONOS_validation"];

pub static REQUIRED_DEVICE_EXTENSIONS: [&str; 2] = ["VK_EXT_memory_budget", "VK_KHR_swapchain"];

pub static REQUIRED_INSTANCE_EXTENSIONS: [&str; 2] = ["VK_KHR_surface", "VK_EXT_debug_utils"];

#[derive(Debug, strum_macros::EnumIter)]
pub enum MemoryKind {
  Buffer1,
  Image1
}