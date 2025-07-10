use derive_new::new;
use crate::gfx_traits::*;
use crate::gfx_traits::HasDevice;

#[derive(new)]
/// collection of vulkan stuff with an effectively 'static' lifetime
pub struct GFX {
  // required for all contexts
  pub entry: ash::Entry,
  pub instance: ash::Instance,
  pub physical_device: ash::vk::PhysicalDevice,
  pub device: ash::Device,
  pub command_pool: ash::vk::CommandPool,
  pub queue_family_index: u32,
  pub main_queue: ash::vk::Queue,

  // required for window context
  pub surface: ash::vk::SurfaceKHR,
  pub surface_instance: ash::khr::surface::Instance,
  pub swapchain: ash::vk::SwapchainKHR,
  pub swapchain_device: ash::khr::swapchain::Device,
  pub display_handle: raw_window_handle::RawDisplayHandle,
  pub window_handle: raw_window_handle::RawWindowHandle,
  pub event_loop: winit::event_loop::EventLoop<()>,
  pub window: winit::window::Window,
}

impl HasDevice for GFX {
  fn device(&self) -> &ash::Device {
    &self.device
  }
}

impl HasEntry for GFX {
  fn entry(&self) -> &ash::Entry {
    &self.entry
  }
}

impl HasInstance for GFX {
  fn instance(&self) -> &ash::Instance {
    &self.instance
  }
}

impl HasPhysicalDevice for GFX {
  fn physical_device(&self) -> &ash::vk::PhysicalDevice {
    &self.physical_device
  }
} 

impl HasCommandPool for GFX {
  fn command_pool(&self) -> &ash::vk::CommandPool {
    &self.command_pool
  }
}

impl HasQueueFamilyIndex for GFX {
  fn queue_family_index(&self) -> u32 {
    self.queue_family_index
  }
}

impl HasMainQueue for GFX {
  fn main_queue(&self) -> &ash::vk::Queue {
    &self.main_queue
  }
}

impl HasSurface for GFX {
  fn surface(&self) -> &ash::vk::SurfaceKHR {
    &self.surface
  }
}

impl HasSurfaceInstance for GFX {
  fn surface_instance(&self) -> &ash::khr::surface::Instance {
    &self.surface_instance
  }
}

impl HasSwapchain for GFX {
  fn swapchain(&self) -> &ash::vk::SwapchainKHR {
    &self.swapchain
  }
}

impl HasSwapchainDevice for GFX {
  fn swapchain_device(&self) -> &ash::khr::swapchain::Device {
    &self.swapchain_device
  }
}

impl HasDisplayHandle for GFX {
  fn display_handle(&self) -> &raw_window_handle::RawDisplayHandle {
    &self.display_handle
  }
}

impl HasWindowHandle for GFX {
  fn window_handle(&self) -> &raw_window_handle::RawWindowHandle {
    &self.window_handle
  }
} 

impl HasEventLoop for GFX {
  fn event_loop(&self) -> &winit::event_loop::EventLoop<()> {
    &self.event_loop
  }
}

impl HasWindow for GFX {
  fn window(&self) -> &winit::window::Window {
    &self.window
  }
}