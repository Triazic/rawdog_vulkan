pub trait HasEntry {
  fn entry(&self) -> &ash::Entry;
}

pub trait HasInstance {
  fn instance(&self) -> &ash::Instance;
}

pub trait HasPhysicalDevice {
  fn physical_device(&self) -> &ash::vk::PhysicalDevice;
}

pub trait HasDevice {
  fn device(&self) -> &ash::Device;
}

pub trait HasCommandPool {
  fn command_pool(&self) -> &ash::vk::CommandPool;
}

pub trait HasQueueFamilyIndex {
  fn queue_family_index(&self) -> u32;
}

pub trait HasMainQueue {
  fn main_queue(&self) -> &ash::vk::Queue;
}

pub trait HasSurface {
  fn surface(&self) -> &ash::vk::SurfaceKHR;
}

pub trait HasSurfaceInstance {
  fn surface_instance(&self) -> &ash::khr::surface::Instance;
}

pub trait HasSwapchain {
  fn swapchain(&self) -> &ash::vk::SwapchainKHR;
}

pub trait HasSwapchainDevice {
  fn swapchain_device(&self) -> &ash::khr::swapchain::Device;
}

pub trait HasDisplayHandle {
  fn display_handle(&self) -> &raw_window_handle::RawDisplayHandle;
}

pub trait HasWindowHandle {
  fn window_handle(&self) -> &raw_window_handle::RawWindowHandle;
}

pub trait HasEventLoop {
  fn event_loop(&self) -> &winit::event_loop::EventLoop<()>;
}

pub trait HasWindow {
  fn window(&self) -> &winit::window::Window;
}