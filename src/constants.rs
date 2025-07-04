pub static API_VERSION: u32 = ash::vk::API_VERSION_1_1;

pub static REQUIRED_INSTANCE_LAYERS: [&str; 1] = ["VK_LAYER_KHRONOS_validation"];

pub static REQUIRED_DEVICE_LAYERS: [&str; 1] = ["VK_LAYER_KHRONOS_validation"];

pub static REQUIRED_DEVICE_EXTENSIONS: [&str; 1] = ["VK_EXT_memory_budget"];

pub const MEMORY_PROPERTY_FLAGS: [ash::vk::MemoryPropertyFlags; 0] = [];

pub const ALL_MEMORY_PROPERTY_FLAGS: [ash::vk::MemoryPropertyFlags; 1] = [ash::vk::MemoryPropertyFlags::DEVICE_LOCAL];

#[derive(Debug, strum_macros::EnumIter)]
pub enum MemoryKind {
    Buffer1,
    Buffer2,
}