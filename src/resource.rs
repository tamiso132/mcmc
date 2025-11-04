//! Translation of resource.hpp
//! Uses 'ash' and 'vk-mem-rs' (with ash feature).
//!
//! FIX: Removed the use of the unstable std::sync::RwLockReadGuard::map.
//! The functions now return the RwLockReadGuard directly, allowing
//! the caller to index into the contained Vec<T> safely.

use crate::bindless::BindlessDescriptors;
use crate::util::{TBufferInfo, TImageInfo, vk_check};
use ash::{Device, vk};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, RwLock, RwLockReadGuard};
use vk_mem::{self as vma, Alloc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    Buffer,
    Image,
}

#[derive(Debug)]
pub struct BufferData {
    pub name: String,
    pub handle: vk::Buffer,
    pub alloc: vma::Allocation,
    pub size: vk::DeviceSize,
    pub offset: vk::DeviceSize,
    pub usage: vk::BufferUsageFlags,
    pub memory_props: vk::MemoryPropertyFlags,
}

#[derive(Debug)]
pub struct ImageData {
    pub name: String,
    pub handle: vk::Image,
    pub view: vk::ImageView,
    pub alloc: Option<vma::Allocation>,
    pub offset: vk::DeviceSize,
    pub format: vk::Format,
    pub extent: vk::Extent3D,
    pub aspect: vk::ImageAspectFlags,
    pub mip_levels: u32,
    pub array_layers: u32,
    pub usage: vk::ImageUsageFlags,
    pub current_layout: vk::ImageLayout,
}

#[derive(Debug, Clone, Copy, Eq)]
pub struct ResourceHandle {
    pub id: u32,
    pub ty: ResourceType,
}
// Manual implementation of PartialEq and Hash to match C++
impl PartialEq for ResourceHandle {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Hash for ResourceHandle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let combined = (self.id << 2) | (self.ty as u32);
        combined.hash(state);
    }
}

pub struct ResourceManager {
    allocator: Arc<vma::Allocator>,
    device: Arc<Device>,
    bindless: BindlessDescriptors,
    buffers: RwLock<Vec<BufferData>>,
    images: RwLock<Vec<ImageData>>,
}

impl ResourceManager {
    pub fn new(allocator: Arc<vma::Allocator>, device: Arc<Device>) -> Self {
        let bindless = BindlessDescriptors::new(device.clone());
        Self {
            allocator,
            device,
            bindless,
            buffers: RwLock::new(Vec::new()),
            images: RwLock::new(Vec::new()),
        }
    }

    pub fn create_buffer(&self, name: String, info: TBufferInfo) -> ResourceHandle {
        unsafe {
            let (buffer, alloc) =
                vk_check!(self.allocator.create_buffer(&info.info, &info.alloc_info));

            let buffer_data = BufferData {
                name,
                handle: buffer,
                alloc,
                size: info.info.size,
                usage: info.info.usage,
                offset: 0 as vk::DeviceSize,
                memory_props: vk::MemoryPropertyFlags::empty(), // VMA handles this
            };

            let mut buffers = self.buffers.write().unwrap();
            let id = buffers.len() as u32;
            buffers.push(buffer_data);

            ResourceHandle {
                id,
                ty: ResourceType::Buffer,
            }
        }
    }

    pub fn create_image(&self, name: String, info: TImageInfo) -> ResourceHandle {
        unsafe {
            let (image, alloc) =
                vk_check!(self.allocator.create_image(&info.info, &info.alloc_info));

            // Create the image view
            let view_info = vk::ImageViewCreateInfo {
                s_type: vk::StructureType::IMAGE_VIEW_CREATE_INFO,
                image,
                view_type: vk::ImageViewType::TYPE_2D,
                format: info.info.format,
                subresource_range: vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    layer_count: 1,
                    level_count: 1,
                    ..Default::default()
                },
                ..Default::default()
            };
            let view = vk_check!(unsafe { self.device.create_image_view(&view_info, None) });

            let image_data = ImageData {
                name,
                handle: image,
                view,
                alloc: Some(alloc), // Managed by VMA
                offset: 0 as vk::DeviceSize,
                format: info.info.format,
                extent: info.info.extent,
                aspect: vk::ImageAspectFlags::COLOR, // Default
                mip_levels: info.info.mip_levels,
                array_layers: info.info.array_layers,
                usage: info.info.usage,
                current_layout: vk::ImageLayout::UNDEFINED,
            };

            let mut images = self.images.write().unwrap();
            let id = images.len() as u32;
            images.push(image_data);

            ResourceHandle {
                id,
                ty: ResourceType::Image,
            }
        }
    }

    pub fn register_image(
        &self,
        info: TImageInfo,
        image: vk::Image,
        view: vk::ImageView,
    ) -> ResourceHandle {
        let image_data = ImageData {
            name: String::new(), // Or add name param
            handle: image,
            view,
            alloc: None, // Not managed by VMA
            offset: 0,
            format: info.info.format,
            extent: info.info.extent,
            aspect: vk::ImageAspectFlags::COLOR, // Default
            mip_levels: info.info.mip_levels,
            array_layers: info.info.array_layers,
            usage: info.info.usage,
            current_layout: vk::ImageLayout::UNDEFINED,
        };

        let mut images = self.images.write().unwrap();
        let id = images.len() as u32;
        images.push(image_data);

        ResourceHandle {
            id,
            ty: ResourceType::Image,
        }
    }

    /// Retrieves the read guard for the vector of buffers.
    /// The caller must now index into the returned guard.
    pub fn get_buffer(&self, _handle: ResourceHandle) -> RwLockReadGuard<Vec<BufferData>> {
        self.buffers.read().unwrap()
    }

    /// Retrieves the read guard for the vector of images.
    /// The caller must now index into the returned guard.
    pub fn get_image(&self, _handle: ResourceHandle) -> RwLockReadGuard<Vec<ImageData>> {
        self.images.read().unwrap()
    }

    pub fn get_layout(&self) -> vk::PipelineLayout {
        self.bindless.pipeline_layout
    }
}

impl Drop for ResourceManager {
    fn drop(&mut self) {
        let mut buffers = self.buffers.write().unwrap();
        for data in buffers.iter_mut() {
            unsafe {
                // Must pass &mut alloc
                self.allocator.destroy_buffer(data.handle, &mut data.alloc);
            }
        }

        let mut images = self.images.write().unwrap();
        for data in images.iter_mut() {
            unsafe { self.device.destroy_image_view(data.view, None) };

            // Only destroy image if it was created by VMA (i.e., alloc is Some)
            if data.alloc.is_some() {
                unsafe {
                    // Must unwrap the Option<Allocation> and pass &mut
                    self.allocator
                        .destroy_image(data.handle, data.alloc.as_mut().unwrap());
                }
            }
        }
    }
}
