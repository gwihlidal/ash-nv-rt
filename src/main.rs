extern crate winit;
#[macro_use]
extern crate ash;
#[cfg(target_os = "windows")]
extern crate winapi;

use ash::extensions::{
    ext::DebugReport,
    khr::{Surface, Swapchain},
};

use ash::extensions::nv::RayTracing;

#[cfg(target_os = "windows")]
use ash::extensions::khr::Win32Surface;

use ash::extensions::nv;
use ash::util::*;
use ash::version::{DeviceV1_0, EntryV1_0, InstanceV1_0};
use ash::{vk, Device, Entry, Instance};
use std::cell::RefCell;
use std::default::Default;
use std::ffi::{CStr, CString};
use std::fs::File;
use std::mem::align_of;
use std::ops::Drop;
use std::os::raw::{c_char, c_void};
use std::path::Path;
use std::rc::Rc;

#[repr(C)]
#[derive(Clone, Debug, Copy)]
struct Vertex {
    pos: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Debug, Copy)]
struct GeometryInstance {
    transform: [f32; 12],
    instance_id_and_mask: u32,
    instance_offset_and_flags: u32,
    acceleration_handle: u64,
}

impl GeometryInstance {
    fn new(
        transform: [f32; 12],
        id: u32,
        mask: u8,
        offset: u32,
        flags: vk::GeometryInstanceFlagsNV,
        acceleration_handle: u64,
    ) -> Self {
        let mut instance = GeometryInstance {
            transform,
            instance_id_and_mask: 0,
            instance_offset_and_flags: 0,
            acceleration_handle,
        };
        instance.set_id(id);
        instance.set_mask(mask);
        instance.set_offset(offset);
        instance.set_flags(flags);
        instance
    }

    fn set_id(&mut self, id: u32) {
        let id = id & 0x00ffffff;
        self.instance_id_and_mask |= id;
    }

    fn set_mask(&mut self, mask: u8) {
        let mask = mask as u32;
        self.instance_id_and_mask |= mask << 24;
    }

    fn set_offset(&mut self, offset: u32) {
        let offset = offset & 0x00ffffff;
        self.instance_offset_and_flags |= offset;
    }

    fn set_flags(&mut self, flags: vk::GeometryInstanceFlagsNV) {
        let flags = flags.as_raw() as u32;
        self.instance_offset_and_flags |= flags << 24;
    }
}

#[derive(Clone)]
struct ImageResource {
    image: vk::Image,
    memory: vk::DeviceMemory,
    view: vk::ImageView,
    sampler: vk::Sampler,
    base: Rc<Base>,
}

impl ImageResource {
    fn new(base: Rc<Base>) -> Self {
        ImageResource {
            image: vk::Image::null(),
            memory: vk::DeviceMemory::null(),
            view: vk::ImageView::null(),
            sampler: vk::Sampler::null(),
            base,
        }
    }

    fn create_image(
        &mut self,
        image_type: vk::ImageType,
        format: vk::Format,
        extent: vk::Extent3D,
        tiling: vk::ImageTiling,
        usage: vk::ImageUsageFlags,
        memory_flags: vk::MemoryPropertyFlags,
    ) {
        unsafe {
            let create_info = vk::ImageCreateInfo::builder()
                .image_type(image_type)
                .format(format)
                .extent(extent)
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(tiling)
                .usage(usage)
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .build();

            self.image = self.base.device.create_image(&create_info, None).unwrap();

            let requirements = self.base.device.get_image_memory_requirements(self.image);
            let memory_index = find_memorytype_index(
                &requirements,
                &self.base.device_memory_properties,
                memory_flags,
            )
            .expect("Unable to find suitable memory index image.");

            let allocate_info = vk::MemoryAllocateInfo {
                allocation_size: requirements.size,
                memory_type_index: memory_index,
                ..Default::default()
            };

            self.memory = self
                .base
                .device
                .allocate_memory(&allocate_info, None)
                .unwrap();

            self.base
                .device
                .bind_image_memory(self.image, self.memory, 0)
                .expect("Unable to bind image memory");
        }
    }

    fn create_view(
        &mut self,
        view_type: vk::ImageViewType,
        format: vk::Format,
        range: vk::ImageSubresourceRange,
    ) {
        let create_info = vk::ImageViewCreateInfo::builder()
            .view_type(view_type)
            .format(format)
            .subresource_range(range)
            .image(self.image)
            .components(vk::ComponentMapping {
                r: vk::ComponentSwizzle::R,
                g: vk::ComponentSwizzle::G,
                b: vk::ComponentSwizzle::B,
                a: vk::ComponentSwizzle::A,
            })
            .build();
        self.view = unsafe {
            self.base
                .device
                .create_image_view(&create_info, None)
                .unwrap()
        };
    }
}

impl Drop for ImageResource {
    fn drop(&mut self) {
        unsafe {
            self.base.device.destroy_image_view(self.view, None);
            self.base.device.free_memory(self.memory, None);
            self.base.device.destroy_image(self.image, None);
            self.base.device.destroy_sampler(self.sampler, None);
        }
    }
}

#[derive(Clone)]
struct BufferResource {
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
    size: vk::DeviceSize,
    base: Rc<Base>,
}

impl BufferResource {
    fn new(
        size: vk::DeviceSize,
        usage: vk::BufferUsageFlags,
        memory_properties: vk::MemoryPropertyFlags,
        base: Rc<Base>,
    ) -> Self {
        unsafe {
            let buffer_info = vk::BufferCreateInfo::builder()
                .size(size)
                .usage(usage)
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .build();

            let buffer = base.device.create_buffer(&buffer_info, None).unwrap();

            let memory_req = base.device.get_buffer_memory_requirements(buffer);

            let memory_index = find_memorytype_index(
                &memory_req,
                &base.device_memory_properties,
                memory_properties,
            )
            .unwrap();

            let allocate_info = vk::MemoryAllocateInfo {
                allocation_size: memory_req.size,
                memory_type_index: memory_index,
                ..Default::default()
            };

            let memory = base.device.allocate_memory(&allocate_info, None).unwrap();

            base.device.bind_buffer_memory(buffer, memory, 0).unwrap();

            BufferResource {
                buffer,
                memory,
                size,
                base,
            }
        }
    }

    fn store<T: Copy>(&mut self, data: &[T]) {
        unsafe {
            let size = (std::mem::size_of::<T>() * data.len()) as u64;
            let mapped_ptr = self.map(size);
            let mut mapped_slice = Align::new(mapped_ptr, align_of::<T>() as u64, size);
            mapped_slice.copy_from_slice(&data);
            self.unmap();
        }
    }

    fn map(&mut self, size: vk::DeviceSize) -> *mut std::ffi::c_void {
        unsafe {
            let data: *mut std::ffi::c_void = self
                .base
                .device
                .map_memory(self.memory, 0, size, vk::MemoryMapFlags::empty())
                .unwrap();
            data
        }
    }

    fn unmap(&mut self) {
        unsafe {
            self.base.device.unmap_memory(self.memory);
        }
    }
}

impl Drop for BufferResource {
    fn drop(&mut self) {
        unsafe {
            self.base.device.destroy_buffer(self.buffer, None);
            self.base.device.free_memory(self.memory, None);
        }
    }
}

#[derive(Clone)]
struct RayTracingApp {
    base: Rc<Base>,
    ray_tracing: Rc<nv::RayTracing>,
    properties: vk::PhysicalDeviceRayTracingPropertiesNV,
    top_as_memory: vk::DeviceMemory,
    top_as: vk::AccelerationStructureNV,
    bottom_as_memory: vk::DeviceMemory,
    bottom_as: vk::AccelerationStructureNV,
    descriptor_set_layout: vk::DescriptorSetLayout,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    shader_binding_table: Option<BufferResource>,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set: vk::DescriptorSet,
    offscreen_target: ImageResource,
    rgen_shader_module: vk::ShaderModule,
    chit_shader_module: vk::ShaderModule,
    miss_shader_module: vk::ShaderModule,
    lib_shader_module: vk::ShaderModule,
}

impl RayTracingApp {
    fn new(
        base: Rc<Base>,
        ray_tracing: Rc<nv::RayTracing>,
        properties: vk::PhysicalDeviceRayTracingPropertiesNV,
    ) -> Self {
        RayTracingApp {
            base: base.clone(),
            ray_tracing,
            properties,
            top_as_memory: vk::DeviceMemory::null(),
            top_as: vk::AccelerationStructureNV::null(),
            bottom_as_memory: vk::DeviceMemory::null(),
            bottom_as: vk::AccelerationStructureNV::null(),
            descriptor_set_layout: vk::DescriptorSetLayout::null(),
            pipeline_layout: vk::PipelineLayout::null(),
            pipeline: vk::Pipeline::null(),
            shader_binding_table: None,
            descriptor_pool: vk::DescriptorPool::null(),
            descriptor_set: vk::DescriptorSet::null(),
            offscreen_target: ImageResource::new(base),
            rgen_shader_module: vk::ShaderModule::null(),
            chit_shader_module: vk::ShaderModule::null(),
            miss_shader_module: vk::ShaderModule::null(),
            lib_shader_module: vk::ShaderModule::null(),
        }
    }

    fn initialize(&mut self) {
        self.create_offscreen_target();
        self.create_acceleration_structures();
        self.create_pipeline();
        self.create_shader_binding_table();
        self.create_descriptor_set();
    }

    fn release(&mut self) {
        unsafe {
            self.base.device.device_wait_idle().unwrap();

            self.ray_tracing
                .destroy_acceleration_structure(self.top_as, None);
            self.base.device.free_memory(self.top_as_memory, None);

            self.ray_tracing
                .destroy_acceleration_structure(self.bottom_as, None);
            self.base.device.free_memory(self.bottom_as_memory, None);

            self.base
                .device
                .destroy_descriptor_pool(self.descriptor_pool, None);

            self.shader_binding_table = None;

            self.base.device.destroy_pipeline(self.pipeline, None);
            self.base
                .device
                .destroy_pipeline_layout(self.pipeline_layout, None);
            self.base
                .device
                .destroy_descriptor_set_layout(self.descriptor_set_layout, None);

            self.base
                .device
                .destroy_shader_module(self.rgen_shader_module, None);
            self.base
                .device
                .destroy_shader_module(self.chit_shader_module, None);
            self.base
                .device
                .destroy_shader_module(self.miss_shader_module, None);
            self.base
                .device
                .destroy_shader_module(self.lib_shader_module, None);
        }
    }

    fn create_offscreen_target(&mut self) {
        self.offscreen_target.create_image(
            vk::ImageType::TYPE_2D,
            self.base.surface_format.format,
            vk::Extent3D::builder()
                .width(1024)
                .height(768)
                .depth(1)
                .build(),
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        );

        self.offscreen_target.create_view(
            vk::ImageViewType::TYPE_2D,
            self.base.surface_format.format,
            vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            },
        );
    }

    fn create_acceleration_structures(&mut self) {
        unsafe {
            // Create geometry

            let vertices = [
                Vertex {
                    pos: [-0.5, -0.5, 0.0],
                },
                Vertex {
                    pos: [0.0, 0.5, 0.0],
                },
                Vertex {
                    pos: [0.5, -0.5, 0.0],
                },
            ];

            let vertex_count = vertices.len();
            let vertex_stride = std::mem::size_of::<Vertex>();

            let vertex_buffer_size = vertex_stride * vertex_count;
            let mut vertex_buffer = BufferResource::new(
                vertex_buffer_size as u64,
                vk::BufferUsageFlags::VERTEX_BUFFER,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                self.base.clone(),
            );
            vertex_buffer.store(&vertices);

            let indices = [0u16, 1, 2];
            let index_count = indices.len();
            let index_buffer_size = std::mem::size_of::<u16>() * index_count;
            let mut index_buffer = BufferResource::new(
                index_buffer_size as u64,
                vk::BufferUsageFlags::INDEX_BUFFER,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                self.base.clone(),
            );
            index_buffer.store(&indices);

            let geometry = vec![vk::GeometryNV::builder()
                .geometry_type(vk::GeometryTypeNV::TRIANGLES)
                .geometry(
                    vk::GeometryDataNV::builder()
                        .triangles(
                            vk::GeometryTrianglesNV::builder()
                                .vertex_data(vertex_buffer.buffer)
                                .vertex_offset(0)
                                .vertex_count(vertex_count as u32)
                                .vertex_stride(vertex_stride as u64)
                                .vertex_format(vk::Format::R32G32B32_SFLOAT)
                                .index_data(index_buffer.buffer)
                                .index_offset(0)
                                .index_count(index_count as u32)
                                .index_type(vk::IndexType::UINT16)
                                .build(),
                        )
                        .build(),
                )
                .flags(vk::GeometryFlagsNV::OPAQUE)
                .build()];

            // Create bottom-level acceleration structure

            let accel_info = vk::AccelerationStructureCreateInfoNV::builder()
                .compacted_size(0)
                .info(
                    vk::AccelerationStructureInfoNV::builder()
                        .ty(vk::AccelerationStructureTypeNV::BOTTOM_LEVEL)
                        .geometries(&geometry)
                        .flags(vk::BuildAccelerationStructureFlagsNV::PREFER_FAST_TRACE)
                        .build(),
                )
                .build();

            self.bottom_as = self
                .ray_tracing
                .create_acceleration_structure(&accel_info, None)
                .unwrap();

            let memory_requirements = self
                .ray_tracing
                .get_acceleration_structure_memory_requirements(
                    &vk::AccelerationStructureMemoryRequirementsInfoNV::builder()
                        .acceleration_structure(self.bottom_as)
                        .ty(vk::AccelerationStructureMemoryRequirementsTypeNV::OBJECT)
                        .build(),
                );

            self.bottom_as_memory = self
                .base
                .device
                .allocate_memory(
                    &vk::MemoryAllocateInfo::builder()
                        .allocation_size(memory_requirements.memory_requirements.size)
                        .memory_type_index(
                            find_memorytype_index(
                                &memory_requirements.memory_requirements,
                                &self.base.device_memory_properties,
                                vk::MemoryPropertyFlags::DEVICE_LOCAL,
                            )
                            .unwrap(),
                        )
                        .build(),
                    None,
                )
                .unwrap();

            self.ray_tracing
                .bind_acceleration_structure_memory(&[
                    vk::BindAccelerationStructureMemoryInfoNV::builder()
                        .acceleration_structure(self.bottom_as)
                        .memory(self.bottom_as_memory)
                        .build(),
                ])
                .unwrap();

            // Create instance buffer

            let accel_handle = self
                .ray_tracing
                .get_acceleration_structure_handle(self.bottom_as)
                .unwrap();

            let transform: [f32; 12] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
            let instance = GeometryInstance::new(
                transform,
                0,
                0xff,
                0,
                vk::GeometryInstanceFlagsNV::TRIANGLE_CULL_DISABLE,
                accel_handle,
            );

            let instance_buffer_size = std::mem::size_of::<GeometryInstance>();
            let mut instance_buffer = BufferResource::new(
                instance_buffer_size as u64,
                vk::BufferUsageFlags::RAY_TRACING_NV,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                self.base.clone(),
            );
            instance_buffer.store(&[instance]);

            // Create top-level acceleration structure

            let accel_info = vk::AccelerationStructureCreateInfoNV::builder()
                .compacted_size(0)
                .info(
                    vk::AccelerationStructureInfoNV::builder()
                        .ty(vk::AccelerationStructureTypeNV::TOP_LEVEL)
                        .instance_count(1)
                        .build(),
                )
                .build();

            self.top_as = self
                .ray_tracing
                .create_acceleration_structure(&accel_info, None)
                .unwrap();

            let memory_requirements = self
                .ray_tracing
                .get_acceleration_structure_memory_requirements(
                    &vk::AccelerationStructureMemoryRequirementsInfoNV::builder()
                        .acceleration_structure(self.top_as)
                        .ty(vk::AccelerationStructureMemoryRequirementsTypeNV::OBJECT)
                        .build(),
                );

            self.top_as_memory = self
                .base
                .device
                .allocate_memory(
                    &vk::MemoryAllocateInfo::builder()
                        .allocation_size(memory_requirements.memory_requirements.size)
                        .memory_type_index(
                            find_memorytype_index(
                                &memory_requirements.memory_requirements,
                                &self.base.device_memory_properties,
                                vk::MemoryPropertyFlags::DEVICE_LOCAL,
                            )
                            .unwrap(),
                        )
                        .build(),
                    None,
                )
                .unwrap();

            self.ray_tracing
                .bind_acceleration_structure_memory(&[
                    vk::BindAccelerationStructureMemoryInfoNV::builder()
                        .acceleration_structure(self.top_as)
                        .memory(self.top_as_memory)
                        .build(),
                ])
                .unwrap();

            // Build acceleration structures

            let bottom_as_size = {
                let requirements = self
                    .ray_tracing
                    .get_acceleration_structure_memory_requirements(
                        &vk::AccelerationStructureMemoryRequirementsInfoNV::builder()
                            .acceleration_structure(self.bottom_as)
                            .ty(vk::AccelerationStructureMemoryRequirementsTypeNV::BUILD_SCRATCH)
                            .build(),
                    );
                requirements.memory_requirements.size
            };

            let top_as_size = {
                let requirements = self
                    .ray_tracing
                    .get_acceleration_structure_memory_requirements(
                        &vk::AccelerationStructureMemoryRequirementsInfoNV::builder()
                            .acceleration_structure(self.top_as)
                            .ty(vk::AccelerationStructureMemoryRequirementsTypeNV::BUILD_SCRATCH)
                            .build(),
                    );
                requirements.memory_requirements.size
            };

            let scratch_buffer_size = std::cmp::max(bottom_as_size, top_as_size);
            let scratch_buffer = BufferResource::new(
                scratch_buffer_size,
                vk::BufferUsageFlags::RAY_TRACING_NV,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
                self.base.clone(),
            );

            let allocate_info = vk::CommandBufferAllocateInfo::builder()
                .command_buffer_count(1)
                .command_pool(self.base.pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .build();

            let command_buffers = self
                .base
                .device
                .allocate_command_buffers(&allocate_info)
                .unwrap();
            let build_command_buffer = command_buffers[0];

            self.base
                .device
                .begin_command_buffer(
                    build_command_buffer,
                    &vk::CommandBufferBeginInfo::builder()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
                        .build(),
                )
                .unwrap();

            let memory_barrier = vk::MemoryBarrier::builder()
                .src_access_mask(
                    vk::AccessFlags::ACCELERATION_STRUCTURE_WRITE_NV
                        | vk::AccessFlags::ACCELERATION_STRUCTURE_READ_NV,
                )
                .dst_access_mask(
                    vk::AccessFlags::ACCELERATION_STRUCTURE_WRITE_NV
                        | vk::AccessFlags::ACCELERATION_STRUCTURE_READ_NV,
                )
                .build();

            self.ray_tracing.cmd_build_acceleration_structure(
                build_command_buffer,
                &vk::AccelerationStructureInfoNV::builder()
                    .ty(vk::AccelerationStructureTypeNV::BOTTOM_LEVEL)
                    .geometries(&geometry)
                    .build(),
                vk::Buffer::null(),
                0,
                false,
                self.bottom_as,
                vk::AccelerationStructureNV::null(),
                scratch_buffer.buffer,
                0,
            );

            self.base.device.cmd_pipeline_barrier(
                build_command_buffer,
                vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_NV,
                vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_NV,
                vk::DependencyFlags::empty(),
                &[memory_barrier],
                &[],
                &[],
            );

            self.ray_tracing.cmd_build_acceleration_structure(
                build_command_buffer,
                &vk::AccelerationStructureInfoNV::builder()
                    .ty(vk::AccelerationStructureTypeNV::TOP_LEVEL)
                    .instance_count(1)
                    .build(),
                instance_buffer.buffer,
                0,
                false,
                self.top_as,
                vk::AccelerationStructureNV::null(),
                scratch_buffer.buffer,
                0,
            );

            self.base.device.cmd_pipeline_barrier(
                build_command_buffer,
                vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_NV,
                vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_NV,
                vk::DependencyFlags::empty(),
                &[memory_barrier],
                &[],
                &[],
            );

            self.base
                .device
                .end_command_buffer(build_command_buffer)
                .unwrap();

            self.base
                .device
                .queue_submit(
                    self.base.present_queue,
                    &[vk::SubmitInfo::builder()
                        .command_buffers(&[build_command_buffer])
                        .build()],
                    vk::Fence::null(),
                )
                .expect("queue submit failed.");

            match self.base.device.queue_wait_idle(self.base.present_queue) {
                Ok(_) => println!("Successfully built acceleration structures"),
                Err(err) => {
                    println!("Failed to build acceleration structures: {:?}", err);
                    panic!("BLAH");
                }
            }

            self.base
                .device
                .free_command_buffers(self.base.pool, &[build_command_buffer]);
        }
    }

    fn create_pipeline(&mut self) {
        unsafe {
            self.descriptor_set_layout = self
                .base
                .device
                .create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::builder()
                        .bindings(&[
                            vk::DescriptorSetLayoutBinding::builder()
                                .descriptor_count(1)
                                .descriptor_type(vk::DescriptorType::ACCELERATION_STRUCTURE_NV)
                                .stage_flags(vk::ShaderStageFlags::RAYGEN_NV)
                                .binding(0)
                                .build(),
                            vk::DescriptorSetLayoutBinding::builder()
                                .descriptor_count(1)
                                .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
                                .stage_flags(vk::ShaderStageFlags::RAYGEN_NV)
                                .binding(1)
                                .build(),
                        ])
                        .build(),
                    None,
                )
                .unwrap();

            let use_lib = false;
            let use_hlsl = true;
            if use_lib && use_hlsl {
                let lib_path = Path::new("shaders/triangle.hlsl_lib.spv");
                let mut lib_file = File::open(lib_path)
                    .expect(&format!("Could not open lib file: {:?}", lib_path));

                let lib_code = read_spv(&mut lib_file)
                    .expect(&format!("Could not load lib file: {:?}", lib_path));
                let lib_shader_info = vk::ShaderModuleCreateInfo::builder().code(&lib_code);
                self.lib_shader_module = self
                    .base
                    .device
                    .create_shader_module(&lib_shader_info, None)
                    .expect("Library shader module error");
            } else {
                let (rgen_path, rchit_path, rmiss_path) = if use_hlsl {
                    (
                        Path::new("shaders/triangle.hlsl_rgen.spv"),
                        Path::new("shaders/triangle.hlsl_rchit.spv"),
                        Path::new("shaders/triangle.hlsl_rmiss.spv"),
                    )
                } else {
                    (
                        Path::new("shaders/triangle.glsl_rgen.spv"),
                        Path::new("shaders/triangle.glsl_rchit.spv"),
                        Path::new("shaders/triangle.glsl_rmiss.spv"),
                    )
                };

                let mut rgen_file = File::open(&rgen_path)
                    .expect(&format!("Could not open rgen file: {:?}", rgen_path));
                let mut rchit_file = File::open(&rchit_path)
                    .expect(&format!("Could not open rchit file: {:?}", rchit_path));
                let mut rmiss_file = File::open(&rmiss_path)
                    .expect(&format!("Could not open rmiss file: {:?}", rmiss_path));

                let rgen_code = read_spv(&mut rgen_file)
                    .expect(&format!("Could not load rgen file: {:?}", rgen_path));
                let rgen_shader_info = vk::ShaderModuleCreateInfo::builder().code(&rgen_code);
                self.rgen_shader_module = self
                    .base
                    .device
                    .create_shader_module(&rgen_shader_info, None)
                    .expect("Failed to create rgen shader module");

                let rchit_code = read_spv(&mut rchit_file)
                    .expect(&format!("Could not load rchit file: {:?}", rchit_file));
                let rchit_shader_info = vk::ShaderModuleCreateInfo::builder().code(&rchit_code);
                self.chit_shader_module = self
                    .base
                    .device
                    .create_shader_module(&rchit_shader_info, None)
                    .expect("Failed to create rchit shader module");

                let rmiss_code = read_spv(&mut rmiss_file)
                    .expect(&format!("Could not load rmiss file: {:?}", rmiss_file));
                let rmiss_shader_info = vk::ShaderModuleCreateInfo::builder().code(&rmiss_code);
                self.miss_shader_module = self
                    .base
                    .device
                    .create_shader_module(&rmiss_shader_info, None)
                    .expect("Failed to create rmiss shader module");
            }

            let layouts = vec![self.descriptor_set_layout];
            let layout_create_info = vk::PipelineLayoutCreateInfo::builder().set_layouts(&layouts);

            self.pipeline_layout = self
                .base
                .device
                .create_pipeline_layout(&layout_create_info, None)
                .unwrap();

            let shader_groups = vec![
                // group0 = [ raygen ]
                vk::RayTracingShaderGroupCreateInfoNV::builder()
                    .ty(vk::RayTracingShaderGroupTypeNV::GENERAL)
                    .general_shader(0)
                    .closest_hit_shader(vk::SHADER_UNUSED_NV)
                    .any_hit_shader(vk::SHADER_UNUSED_NV)
                    .intersection_shader(vk::SHADER_UNUSED_NV)
                    .build(),
                // group1 = [ chit ]
                vk::RayTracingShaderGroupCreateInfoNV::builder()
                    .ty(vk::RayTracingShaderGroupTypeNV::TRIANGLES_HIT_GROUP)
                    .general_shader(vk::SHADER_UNUSED_NV)
                    .closest_hit_shader(1)
                    .any_hit_shader(vk::SHADER_UNUSED_NV)
                    .intersection_shader(vk::SHADER_UNUSED_NV)
                    .build(),
                // group2 = [ miss ]
                vk::RayTracingShaderGroupCreateInfoNV::builder()
                    .ty(vk::RayTracingShaderGroupTypeNV::GENERAL)
                    .general_shader(2)
                    .closest_hit_shader(vk::SHADER_UNUSED_NV)
                    .any_hit_shader(vk::SHADER_UNUSED_NV)
                    .intersection_shader(vk::SHADER_UNUSED_NV)
                    .build(),
            ];

            let shader_stages = if use_lib && use_hlsl {
                vec![
                    vk::PipelineShaderStageCreateInfo::builder()
                        .stage(vk::ShaderStageFlags::RAYGEN_NV)
                        .module(self.lib_shader_module)
                        .name(std::ffi::CStr::from_bytes_with_nul(b"rgen_main\0").unwrap())
                        .build(),
                    vk::PipelineShaderStageCreateInfo::builder()
                        .stage(vk::ShaderStageFlags::CLOSEST_HIT_NV)
                        .module(self.lib_shader_module)
                        .name(std::ffi::CStr::from_bytes_with_nul(b"rchit_main\0").unwrap())
                        .build(),
                    vk::PipelineShaderStageCreateInfo::builder()
                        .stage(vk::ShaderStageFlags::MISS_NV)
                        .module(self.lib_shader_module)
                        .name(std::ffi::CStr::from_bytes_with_nul(b"rmiss_main\0").unwrap())
                        .build(),
                ]
            } else {
                vec![
                    vk::PipelineShaderStageCreateInfo::builder()
                        .stage(vk::ShaderStageFlags::RAYGEN_NV)
                        .module(self.rgen_shader_module)
                        .name(std::ffi::CStr::from_bytes_with_nul(b"main\0").unwrap())
                        .build(),
                    vk::PipelineShaderStageCreateInfo::builder()
                        .stage(vk::ShaderStageFlags::CLOSEST_HIT_NV)
                        .module(self.chit_shader_module)
                        .name(std::ffi::CStr::from_bytes_with_nul(b"main\0").unwrap())
                        .build(),
                    vk::PipelineShaderStageCreateInfo::builder()
                        .stage(vk::ShaderStageFlags::MISS_NV)
                        .module(self.miss_shader_module)
                        .name(std::ffi::CStr::from_bytes_with_nul(b"main\0").unwrap())
                        .build(),
                ]
            };

            self.pipeline = self
                .ray_tracing
                .create_ray_tracing_pipelines(
                    vk::PipelineCache::null(),
                    &[vk::RayTracingPipelineCreateInfoNV::builder()
                        .stages(&shader_stages)
                        .groups(&shader_groups)
                        .max_recursion_depth(1)
                        .layout(self.pipeline_layout)
                        .build()],
                    None,
                )
                .unwrap()[0];
        }
    }

    fn create_shader_binding_table(&mut self) {
        let group_count = 3; // Listed in vk::RayTracingPipelineCreateInfoNV
        let table_size = (self.properties.shader_group_handle_size * group_count) as u64;
        let mut table_data: Vec<u8> = vec![0u8; table_size as usize];
        unsafe {
            self.ray_tracing
                .get_ray_tracing_shader_group_handles(
                    self.pipeline,
                    0,
                    group_count,
                    &mut table_data,
                )
                .unwrap();
        }
        let mut shader_binding_table = BufferResource::new(
            table_size,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE,
            self.base.clone(),
        );
        shader_binding_table.store(&table_data);
        self.shader_binding_table = Some(shader_binding_table);
    }

    fn create_descriptor_set(&mut self) {
        unsafe {
            let descriptor_sizes = [
                vk::DescriptorPoolSize {
                    ty: vk::DescriptorType::ACCELERATION_STRUCTURE_NV,
                    descriptor_count: 1,
                },
                vk::DescriptorPoolSize {
                    ty: vk::DescriptorType::STORAGE_IMAGE,
                    descriptor_count: 1,
                },
            ];

            let descriptor_pool_info = vk::DescriptorPoolCreateInfo::builder()
                .pool_sizes(&descriptor_sizes)
                .max_sets(1);

            self.descriptor_pool = self
                .base
                .device
                .create_descriptor_pool(&descriptor_pool_info, None)
                .unwrap();

            let descriptor_sets = self
                .base
                .device
                .allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::builder()
                        .descriptor_pool(self.descriptor_pool)
                        .set_layouts(&[self.descriptor_set_layout])
                        .build(),
                )
                .unwrap();
            self.descriptor_set = descriptor_sets[0];

            let accel_structs = [self.top_as];
            let mut accel_info = vk::WriteDescriptorSetAccelerationStructureNV::builder()
                .acceleration_structures(&accel_structs)
                .build();

            let mut accel_write = vk::WriteDescriptorSet::builder()
                .dst_set(self.descriptor_set)
                .dst_binding(0)
                .dst_array_element(0)
                .descriptor_type(vk::DescriptorType::ACCELERATION_STRUCTURE_NV)
                .next(&mut accel_info)
                .build();

            // This is only set by the builder for images, buffers, or views; need to set explicitly after
            accel_write.descriptor_count = 1;

            let image_info = vk::DescriptorImageInfo::builder()
                .image_layout(vk::ImageLayout::GENERAL)
                .image_view(self.offscreen_target.view)
                .build();

            let image_write = vk::WriteDescriptorSet::builder()
                .dst_set(self.descriptor_set)
                .dst_binding(1)
                .dst_array_element(0)
                .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
                .image_info(&[image_info])
                .build();

            self.base
                .device
                .update_descriptor_sets(&[accel_write, image_write], &[]);
        }
    }

    fn record_image_barrier(
        &self,
        command_buffer: vk::CommandBuffer,
        image: vk::Image,
        range: vk::ImageSubresourceRange,
        src_access: vk::AccessFlags,
        dst_access: vk::AccessFlags,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
    ) {
        let barrier = vk::ImageMemoryBarrier::builder()
            .src_access_mask(src_access)
            .dst_access_mask(dst_access)
            .old_layout(old_layout)
            .new_layout(new_layout)
            .image(image)
            .subresource_range(range)
            .build();
        unsafe {
            self.base.device.cmd_pipeline_barrier(
                command_buffer,
                vk::PipelineStageFlags::ALL_COMMANDS,
                vk::PipelineStageFlags::ALL_COMMANDS,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );
        }
    }

    fn record_command_buffer(&self, present_image: vk::Image, command_buffer: vk::CommandBuffer) {
        let range = vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        };

        self.record_image_barrier(
            command_buffer,
            self.offscreen_target.image,
            range.clone(),
            vk::AccessFlags::empty(),
            vk::AccessFlags::SHADER_WRITE,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::GENERAL,
        );

        self.record_ray_tracing(command_buffer);

        self.record_image_barrier(
            command_buffer,
            present_image,
            range.clone(),
            vk::AccessFlags::empty(),
            vk::AccessFlags::TRANSFER_WRITE,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        );

        self.record_image_barrier(
            command_buffer,
            self.offscreen_target.image,
            range.clone(),
            vk::AccessFlags::SHADER_WRITE,
            vk::AccessFlags::TRANSFER_READ,
            vk::ImageLayout::GENERAL,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        );

        let region = vk::ImageCopy::builder()
            .src_subresource(
                vk::ImageSubresourceLayers::builder()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .layer_count(1)
                    .build(),
            )
            .dst_subresource(
                vk::ImageSubresourceLayers::builder()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .layer_count(1)
                    .build(),
            )
            .extent(
                vk::Extent3D::builder()
                    .width(1024)
                    .height(768)
                    .depth(1)
                    .build(),
            )
            .build();

        unsafe {
            self.base.device.cmd_copy_image(
                command_buffer,
                self.offscreen_target.image,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                present_image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[region],
            );
        }

        self.record_image_barrier(
            command_buffer,
            present_image,
            range.clone(),
            vk::AccessFlags::TRANSFER_WRITE,
            vk::AccessFlags::empty(),
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::PRESENT_SRC_KHR,
        );
    }

    fn record_ray_tracing(&self, command_buffer: vk::CommandBuffer) {
        if let Some(ref shader_binding_table) = self.shader_binding_table {
            let handle_size = self.properties.shader_group_handle_size as u64;

            // |[ raygen shader ]|[ hit shader  ]|[ miss shader ]|
            // |                 |               |               |
            // | 0               | 1             | 2             | 3

            let sbt_raygen_buffer = shader_binding_table.buffer;
            let sbt_raygen_offset = 0;

            let sbt_miss_buffer = shader_binding_table.buffer;
            let sbt_miss_offset = 2 * handle_size;
            let sbt_miss_stride = handle_size;

            let sbt_hit_buffer = shader_binding_table.buffer;
            let sbt_hit_offset = 1 * handle_size;
            let sbt_hit_stride = handle_size;

            let sbt_call_buffer = vk::Buffer::null();
            let sbt_call_offset = 0;
            let sbt_call_stride = 0;

            unsafe {
                self.base.device.cmd_bind_pipeline(
                    command_buffer,
                    vk::PipelineBindPoint::RAY_TRACING_NV,
                    self.pipeline,
                );
                self.base.device.cmd_bind_descriptor_sets(
                    command_buffer,
                    vk::PipelineBindPoint::RAY_TRACING_NV,
                    self.pipeline_layout,
                    0,
                    &[self.descriptor_set],
                    &[],
                );
                self.ray_tracing.cmd_trace_rays(
                    command_buffer,
                    sbt_raygen_buffer,
                    sbt_raygen_offset,
                    sbt_miss_buffer,
                    sbt_miss_offset,
                    sbt_miss_stride,
                    sbt_hit_buffer,
                    sbt_hit_offset,
                    sbt_hit_stride,
                    sbt_call_buffer,
                    sbt_call_offset,
                    sbt_call_stride,
                    1024,
                    768,
                    1,
                )
            }
        }
    }
}

fn main() {
    unsafe {
        let base = Rc::new(Base::new(1024, 768));
        let props_rt = nv::RayTracing::get_properties(&base.instance, base.pdevice);
        let ray_tracing = Rc::new(nv::RayTracing::new(&base.instance, &base.device));
        let mut app = RayTracingApp::new(base.clone(), ray_tracing, props_rt);
        app.initialize();

        println!("NV Ray Tracing Properties:");
        println!(
            " shader_group_handle_size: {}",
            props_rt.shader_group_handle_size
        );
        println!(" max_recursion_depth: {}", props_rt.max_recursion_depth);
        println!(
            " max_shader_group_stride: {}",
            props_rt.max_shader_group_stride
        );
        println!(
            " shader_group_base_alignment: {}",
            props_rt.shader_group_base_alignment
        );
        println!(" max_geometry_count: {}", props_rt.max_geometry_count);
        println!(" max_instance_count: {}", props_rt.max_instance_count);
        println!(" max_triangle_count: {}", props_rt.max_triangle_count);
        println!(
            " max_descriptor_set_acceleration_structures: {}",
            props_rt.max_descriptor_set_acceleration_structures
        );

        base.render_loop(|| {
            let (present_index, _) = base
                .swapchain_loader
                .acquire_next_image(
                    base.swapchain,
                    std::u64::MAX,
                    base.present_complete_semaphore,
                    vk::Fence::null(),
                )
                .unwrap();

            record_submit_commandbuffer(
                &base.device,
                base.command_buffer,
                base.present_queue,
                &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT],
                &[base.present_complete_semaphore],
                &[base.rendering_complete_semaphore],
                |_device, command_buffer| {
                    let present_image = base.present_images[present_index as usize];
                    app.record_command_buffer(present_image, command_buffer);
                },
            );

            let wait_semaphores = [base.rendering_complete_semaphore];
            let swapchains = [base.swapchain];
            let image_indices = [present_index];
            let present_info = vk::PresentInfoKHR::builder()
                .wait_semaphores(&wait_semaphores)
                .swapchains(&swapchains)
                .image_indices(&image_indices);

            base.swapchain_loader
                .queue_present(base.present_queue, &present_info)
                .unwrap();
        });

        base.device.device_wait_idle().unwrap();
        app.release();
    }
}

pub fn record_submit_commandbuffer<D: DeviceV1_0, F: FnOnce(&D, vk::CommandBuffer)>(
    device: &D,
    command_buffer: vk::CommandBuffer,
    submit_queue: vk::Queue,
    wait_mask: &[vk::PipelineStageFlags],
    wait_semaphores: &[vk::Semaphore],
    signal_semaphores: &[vk::Semaphore],
    f: F,
) {
    unsafe {
        device
            .reset_command_buffer(
                command_buffer,
                vk::CommandBufferResetFlags::RELEASE_RESOURCES,
            )
            .expect("Reset command buffer failed.");

        let command_buffer_begin_info = vk::CommandBufferBeginInfo::builder()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        device
            .begin_command_buffer(command_buffer, &command_buffer_begin_info)
            .expect("Begin commandbuffer");
        f(device, command_buffer);
        device
            .end_command_buffer(command_buffer)
            .expect("End commandbuffer");

        let submit_fence = device
            .create_fence(&vk::FenceCreateInfo::default(), None)
            .expect("Create fence failed.");

        let command_buffers = vec![command_buffer];

        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(wait_semaphores)
            .wait_dst_stage_mask(wait_mask)
            .command_buffers(&command_buffers)
            .signal_semaphores(signal_semaphores);

        device
            .queue_submit(submit_queue, &[submit_info.build()], submit_fence)
            .expect("queue submit failed.");
        device
            .wait_for_fences(&[submit_fence], true, std::u64::MAX)
            .expect("Wait for fence failed.");
        device.destroy_fence(submit_fence, None);
    }
}

#[cfg(target_os = "windows")]
unsafe fn create_surface<E: EntryV1_0, I: InstanceV1_0>(
    entry: &E,
    instance: &I,
    window: &winit::Window,
) -> Result<vk::SurfaceKHR, vk::Result> {
    use std::ptr;
    use winapi::shared::windef::HWND;
    use winapi::um::libloaderapi::GetModuleHandleW;
    use winit::os::windows::WindowExt;

    let hwnd = window.get_hwnd() as HWND;
    let hinstance = GetModuleHandleW(ptr::null()) as *const c_void;
    let win32_create_info = vk::Win32SurfaceCreateInfoKHR {
        s_type: vk::StructureType::WIN32_SURFACE_CREATE_INFO_KHR,
        p_next: ptr::null(),
        flags: Default::default(),
        hinstance: hinstance,
        hwnd: hwnd as *const c_void,
    };
    let win32_surface_loader = Win32Surface::new(entry, instance);
    win32_surface_loader.create_win32_surface(&win32_create_info, None)
}

#[cfg(all(unix, not(target_os = "android"), not(target_os = "macos")))]
fn extension_names() -> Vec<*const i8> {
    vec![
        Surface::name().as_ptr(),
        XlibSurface::name().as_ptr(),
        DebugReport::name().as_ptr(),
        vk::KhrGetPhysicalDeviceProperties2Fn::name().as_ptr(),
    ]
}

#[cfg(all(windows))]
fn extension_names() -> Vec<*const i8> {
    vec![
        Surface::name().as_ptr(),
        Win32Surface::name().as_ptr(),
        DebugReport::name().as_ptr(),
        vk::KhrGetPhysicalDeviceProperties2Fn::name().as_ptr(),
    ]
}

unsafe extern "system" fn vulkan_debug_callback(
    _: vk::DebugReportFlagsEXT,
    _: vk::DebugReportObjectTypeEXT,
    _: u64,
    _: usize,
    _: i32,
    _: *const c_char,
    p_message: *const c_char,
    _: *mut c_void,
) -> u32 {
    println!("{:?}", CStr::from_ptr(p_message));
    vk::FALSE
}

pub fn find_memorytype_index(
    memory_req: &vk::MemoryRequirements,
    memory_prop: &vk::PhysicalDeviceMemoryProperties,
    flags: vk::MemoryPropertyFlags,
) -> Option<u32> {
    // Try to find an exactly matching memory flag
    let best_suitable_index =
        find_memorytype_index_f(memory_req, memory_prop, flags, |property_flags, flags| {
            property_flags == flags
        });
    if best_suitable_index.is_some() {
        return best_suitable_index;
    }
    // Otherwise find a memory flag that works
    find_memorytype_index_f(memory_req, memory_prop, flags, |property_flags, flags| {
        property_flags & flags == flags
    })
}

pub fn find_memorytype_index_f<F: Fn(vk::MemoryPropertyFlags, vk::MemoryPropertyFlags) -> bool>(
    memory_req: &vk::MemoryRequirements,
    memory_prop: &vk::PhysicalDeviceMemoryProperties,
    flags: vk::MemoryPropertyFlags,
    f: F,
) -> Option<u32> {
    let mut memory_type_bits = memory_req.memory_type_bits;
    for (index, ref memory_type) in memory_prop.memory_types.iter().enumerate() {
        if memory_type_bits & 1 == 1 {
            if f(memory_type.property_flags, flags) {
                return Some(index as u32);
            }
        }
        memory_type_bits = memory_type_bits >> 1;
    }
    None
}

pub struct Base {
    pub entry: Entry,
    pub instance: Instance,
    pub device: Device,
    pub surface_loader: Surface,
    pub swapchain_loader: Swapchain,
    pub debug_report_loader: DebugReport,
    pub window: winit::Window,
    pub events_loop: RefCell<winit::EventsLoop>,
    pub debug_call_back: vk::DebugReportCallbackEXT,

    pub pdevice: vk::PhysicalDevice,
    pub device_memory_properties: vk::PhysicalDeviceMemoryProperties,
    pub queue_family_index: u32,
    pub present_queue: vk::Queue,

    pub surface: vk::SurfaceKHR,
    pub surface_format: vk::SurfaceFormatKHR,
    pub surface_resolution: vk::Extent2D,

    pub swapchain: vk::SwapchainKHR,
    pub present_images: Vec<vk::Image>,

    pub pool: vk::CommandPool,
    pub command_buffer: vk::CommandBuffer,

    pub present_complete_semaphore: vk::Semaphore,
    pub rendering_complete_semaphore: vk::Semaphore,
}

impl Base {
    pub fn render_loop<F: Fn()>(&self, f: F) {
        use winit::*;
        self.events_loop.borrow_mut().run_forever(|event| {
            f();
            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::KeyboardInput { input, .. } => {
                        if let Some(VirtualKeyCode::Escape) = input.virtual_keycode {
                            ControlFlow::Break
                        } else {
                            ControlFlow::Continue
                        }
                    }
                    WindowEvent::CloseRequested => winit::ControlFlow::Break,
                    _ => ControlFlow::Continue,
                },
                _ => ControlFlow::Continue,
            }
        });
    }

    pub fn new(window_width: u32, window_height: u32) -> Self {
        unsafe {
            let events_loop = winit::EventsLoop::new();
            let window = winit::WindowBuilder::new()
                .with_title("Rust Vulkan NV Ray Tracing w/ HLSL")
                .with_dimensions(winit::dpi::LogicalSize::new(
                    window_width as f64,
                    window_height as f64,
                ))
                .build(&events_loop)
                .unwrap();
            let entry = Entry::new().unwrap();
            let app_name = CString::new("Rust_VK_RT_HLSL").unwrap();

            let layer_names = [CString::new("VK_LAYER_LUNARG_standard_validation").unwrap()];
            let layers_names_raw: Vec<*const i8> = layer_names
                .iter()
                .map(|raw_name| raw_name.as_ptr())
                .collect();

            let extension_names_raw = extension_names();

            let appinfo = vk::ApplicationInfo::builder()
                .application_name(&app_name)
                .application_version(0)
                .engine_name(&app_name)
                .engine_version(0)
                .api_version(vk_make_version!(1, 1, 0));

            let create_info = vk::InstanceCreateInfo::builder()
                .application_info(&appinfo)
                .enabled_layer_names(&layers_names_raw)
                .enabled_extension_names(&extension_names_raw);

            let instance: Instance = entry
                .create_instance(&create_info, None)
                .expect("Instance creation error");

            let debug_info = vk::DebugReportCallbackCreateInfoEXT::builder()
                .flags(
                    vk::DebugReportFlagsEXT::ERROR
                        | vk::DebugReportFlagsEXT::WARNING
                        | vk::DebugReportFlagsEXT::PERFORMANCE_WARNING,
                )
                .pfn_callback(Some(vulkan_debug_callback));

            let debug_report_loader = DebugReport::new(&entry, &instance);
            let debug_call_back = debug_report_loader
                .create_debug_report_callback(&debug_info, None)
                .unwrap();
            let surface = create_surface(&entry, &instance, &window).unwrap();
            let pdevices = instance
                .enumerate_physical_devices()
                .expect("Physical device error");
            let surface_loader = Surface::new(&entry, &instance);
            let (pdevice, queue_family_index) = pdevices
                .iter()
                .map(|pdevice| {
                    instance
                        .get_physical_device_queue_family_properties(*pdevice)
                        .iter()
                        .enumerate()
                        .filter_map(|(index, ref info)| {
                            let supports_graphic_and_surface =
                                info.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                                    && surface_loader.get_physical_device_surface_support(
                                        *pdevice,
                                        index as u32,
                                        surface,
                                    );
                            match supports_graphic_and_surface {
                                true => Some((*pdevice, index)),
                                _ => None,
                            }
                        })
                        .nth(0)
                })
                .filter_map(|v| v)
                .nth(0)
                .expect("Couldn't find suitable device.");
            let queue_family_index = queue_family_index as u32;

            let device_extension_names_raw = vec![
                Swapchain::name().as_ptr(),
                RayTracing::name().as_ptr(),
                vk::ExtDescriptorIndexingFn::name().as_ptr(),
                vk::KhrGetMemoryRequirements2Fn::name().as_ptr(),
            ];

            let features = vk::PhysicalDeviceFeatures {
                shader_clip_distance: 1,
                vertex_pipeline_stores_and_atomics: 1,
                ..Default::default()
            };
            let priorities = [1.0];

            let queue_info = [vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(queue_family_index)
                .queue_priorities(&priorities)
                .build()];

            let mut descriptor_indexing =
                vk::PhysicalDeviceDescriptorIndexingFeaturesEXT::default();
            let mut features2 = vk::PhysicalDeviceFeatures2::builder()
                .next(&mut descriptor_indexing)
                .build();
            features2.features = features;

            let device_create_info = vk::DeviceCreateInfo::builder()
                .queue_create_infos(&queue_info)
                .enabled_extension_names(&device_extension_names_raw)
                .next(&mut features2);

            let device: Device = instance
                .create_device(pdevice, &device_create_info, None)
                .unwrap();

            let present_queue = device.get_device_queue(queue_family_index as u32, 0);

            let surface_formats = surface_loader
                .get_physical_device_surface_formats(pdevice, surface)
                .unwrap();
            let surface_format = surface_formats
                .iter()
                .map(|sfmt| match sfmt.format {
                    vk::Format::UNDEFINED => vk::SurfaceFormatKHR {
                        format: vk::Format::B8G8R8_UNORM,
                        color_space: sfmt.color_space,
                    },
                    _ => sfmt.clone(),
                })
                .nth(0)
                .expect("Unable to find suitable surface format.");
            let surface_capabilities = surface_loader
                .get_physical_device_surface_capabilities(pdevice, surface)
                .unwrap();
            let mut desired_image_count = surface_capabilities.min_image_count + 1;
            if surface_capabilities.max_image_count > 0
                && desired_image_count > surface_capabilities.max_image_count
            {
                desired_image_count = surface_capabilities.max_image_count;
            }
            let surface_resolution = match surface_capabilities.current_extent.width {
                std::u32::MAX => vk::Extent2D {
                    width: window_width,
                    height: window_height,
                },
                _ => surface_capabilities.current_extent,
            };
            let pre_transform = if surface_capabilities
                .supported_transforms
                .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
            {
                vk::SurfaceTransformFlagsKHR::IDENTITY
            } else {
                surface_capabilities.current_transform
            };
            let present_modes = surface_loader
                .get_physical_device_surface_present_modes(pdevice, surface)
                .unwrap();
            let present_mode = present_modes
                .iter()
                .cloned()
                .find(|&mode| mode == vk::PresentModeKHR::MAILBOX)
                .unwrap_or(vk::PresentModeKHR::FIFO);
            let swapchain_loader = Swapchain::new(&instance, &device);

            let swapchain_create_info = vk::SwapchainCreateInfoKHR::builder()
                .surface(surface)
                .min_image_count(desired_image_count)
                .image_color_space(surface_format.color_space)
                .image_format(surface_format.format)
                .image_extent(surface_resolution.clone())
                .image_usage(vk::ImageUsageFlags::TRANSFER_DST)
                .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                .pre_transform(pre_transform)
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .present_mode(present_mode)
                .clipped(true)
                .image_array_layers(1);

            let swapchain = swapchain_loader
                .create_swapchain(&swapchain_create_info, None)
                .unwrap();

            let pool_create_info = vk::CommandPoolCreateInfo::builder()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                .queue_family_index(queue_family_index);

            let pool = device.create_command_pool(&pool_create_info, None).unwrap();

            let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::builder()
                .command_buffer_count(1)
                .command_pool(pool)
                .level(vk::CommandBufferLevel::PRIMARY);

            let command_buffer = device
                .allocate_command_buffers(&command_buffer_allocate_info)
                .unwrap()[0];

            let present_images = swapchain_loader.get_swapchain_images(swapchain).unwrap();
            let device_memory_properties = instance.get_physical_device_memory_properties(pdevice);

            let semaphore_create_info = vk::SemaphoreCreateInfo::default();

            let present_complete_semaphore = device
                .create_semaphore(&semaphore_create_info, None)
                .unwrap();
            let rendering_complete_semaphore = device
                .create_semaphore(&semaphore_create_info, None)
                .unwrap();
            Base {
                events_loop: RefCell::new(events_loop),
                entry,
                instance,
                device,
                queue_family_index,
                pdevice,
                device_memory_properties,
                window,
                surface_loader,
                surface_format,
                present_queue,
                surface_resolution,
                swapchain_loader,
                swapchain,
                present_images,
                pool,
                command_buffer,
                present_complete_semaphore,
                rendering_complete_semaphore,
                surface,
                debug_call_back,
                debug_report_loader,
            }
        }
    }
}

impl Drop for Base {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().unwrap();
            self.device
                .destroy_semaphore(self.present_complete_semaphore, None);
            self.device
                .destroy_semaphore(self.rendering_complete_semaphore, None);
            self.device.destroy_command_pool(self.pool, None);
            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);
            self.device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
            self.debug_report_loader
                .destroy_debug_report_callback(self.debug_call_back, None);
            self.instance.destroy_instance(None);
        }
    }
}
