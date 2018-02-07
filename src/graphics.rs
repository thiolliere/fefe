use vulkano::device::{Device, DeviceExtensions, Queue};
use vulkano::swapchain::{self, Swapchain, SwapchainCreationError};
use vulkano::sampler::{Filter, MipmapMode, Sampler, SamplerAddressMode};
use vulkano::image::{AttachmentImage, Dimensions, ImageUsage, ImmutableImage};
use vulkano::buffer::{BufferUsage, CpuBufferPool, ImmutableBuffer};
use vulkano::framebuffer::{Framebuffer, FramebufferAbstract, LayoutAttachmentDescription,
                           LayoutPassDependencyDescription, LayoutPassDescription, LoadOp,
                           RenderPassAbstract, RenderPassDesc,
                           RenderPassDescClearValues, StoreOp};
use vulkano::pipeline::GraphicsPipelineAbstract;
use vulkano::pipeline::viewport::Viewport;
use vulkano::descriptor::descriptor_set::{DescriptorSet, FixedSizeDescriptorSetsPool,
                                          PersistentDescriptorSet};
use vulkano::command_buffer::pool::standard::StandardCommandPoolAlloc;
use vulkano::command_buffer::{AutoCommandBuffer, AutoCommandBufferBuilder, DynamicState};
use vulkano::instance::PhysicalDevice;
use vulkano::sync::{now, GpuFuture};
use vulkano::image::ImageLayout;
use vulkano::format::{self, ClearValue, Format};
use vulkano;
use alga::general::SubsetOf;

use std::sync::Arc;
use std::fs::File;
use std::time::Duration;

// TODO: only a bool for whereas draw the cursor or not

pub struct Camera {
    position: ::na::Isometry2<f32>,
}

impl Camera {
    pub fn new(position: ::na::Isometry2<f32>) -> Self {
        Camera {
            position,
        }
    }
}

impl Camera {
    fn matrix(&self, dimensions: [u32; 2]) -> [[f32;4];4] {
        let rescale_trans = {
            let ratio = dimensions[0] as f32/ dimensions[1] as f32;

            let (kx, ky) = if ratio > 1. {
                (1.0 / (::CFG.zoom * ratio),
                 1.0 / ::CFG.zoom)
            } else {
                (1.0 / ::CFG.zoom,
                 ratio / ::CFG.zoom)
            };

            let mut trans: ::na::Transform3<f32> = ::na::one();
            trans[(0,0)] = kx;
            trans[(1,1)] = ky;
            trans
        };

        let world_trans: ::na::Transform3<f32> = ::na::Isometry3::<f32>::new(
            ::na::Vector3::new(self.position.translation.vector[0], -self.position.translation.vector[1], 0.0),
            ::na::Vector3::new(0.0, 0.0, -self.position.rotation.angle()),
        ).inverse().to_superset();

        (rescale_trans * world_trans).unwrap().into()
    }
}

struct Image {
    descriptor_set: Arc<DescriptorSet + Sync + Send>,
    width: u32,
    height: u32,
}

pub struct Graphics<'a> {
    physical: PhysicalDevice<'a>,
    queue: Arc<Queue>,
    device: Arc<Device>,
    swapchain: Arc<Swapchain>,
    render_pass: Arc<RenderPassAbstract + Sync + Send>,
    pipeline: Arc<GraphicsPipelineAbstract + Sync + Send>,
    vertex_buffer: Arc<ImmutableBuffer<[Vertex]>>,
    animation_images: Vec<Image>,
    framebuffers: Vec<Arc<FramebufferAbstract + Sync + Send>>,
    view_buffer_pool: CpuBufferPool<vs::ty::View>,
    world_buffer_pool: CpuBufferPool<vs::ty::World>,
    descriptor_sets_pool: FixedSizeDescriptorSetsPool<Arc<GraphicsPipelineAbstract + Sync + Send>>,
    future: Option<Box<GpuFuture>>,
}

#[derive(Debug, Clone)]
struct Vertex {
    position: [f32; 2],
}
impl_vertex!(Vertex, position);

impl<'a> Graphics<'a> {
    pub fn new(window: &'a ::vulkano_win::Window) -> Graphics<'a> {
        let physical = PhysicalDevice::enumerate(&window.surface().instance())
            .next()
            .expect("no device available");

        let queue_family = physical
            .queue_families()
            .find(|&q| {
                q.supports_graphics() && q.supports_compute()
                    && window.surface().is_supported(q).unwrap_or(false)
            })
            .expect("couldn't find a graphical queue family");

        let (device, mut queues) = {
            let device_ext = DeviceExtensions {
                khr_swapchain: true,
                ..DeviceExtensions::none()
            };

            Device::new(
                physical,
                physical.supported_features(),
                &device_ext,
                [(queue_family, 0.5)].iter().cloned(),
            ).expect("failed to create device")
        };

        let queue = queues.next().unwrap();

        let (swapchain, images) = {
            let caps = window
                .surface()
                .capabilities(physical)
                .expect("failed to get surface capabilities");

            let dimensions = caps.current_extent.unwrap_or([1280, 1024]);
            let format = caps.supported_formats[0].0;
            let image_usage = ImageUsage {
                color_attachment: true,
                ..ImageUsage::none()
            };

            Swapchain::new(
                device.clone(),
                window.surface().clone(),
                caps.min_image_count,
                format,
                dimensions,
                1,
                image_usage,
                &queue,
                swapchain::SurfaceTransform::Identity,
                swapchain::CompositeAlpha::Opaque,
                swapchain::PresentMode::Fifo,
                true,
                None,
            ).expect("failed to create swapchain")
        };

        let render_pass = Arc::new(
            CustomRenderPassDesc {
                swapchain_image_format: swapchain.format(),
            }.build_render_pass(device.clone())
                .unwrap(),
        );

        let mut future = Box::new(now(device.clone())) as Box<GpuFuture>;

        let (vertex_buffer, vertex_buffer_fut) = ImmutableBuffer::from_iter(
            [
                [-0.5f32, -0.5],
                [-0.5, 0.5],
                [0.5, -0.5],
                [0.5, 0.5],
                [-0.5, 0.5],
                [0.5, -0.5],
            ].iter()
                .cloned()
                .map(|position| Vertex { position }),
            BufferUsage::vertex_buffer(),
            queue.clone(),
        ).expect("failed to create buffer");
        future = Box::new(future.join(vertex_buffer_fut)) as Box<_>;

        let vs = vs::Shader::load(device.clone()).expect("failed to create shader module");
        let fs = fs::Shader::load(device.clone()).expect("failed to create shader module");

        let pipeline = Arc::new(
            vulkano::pipeline::GraphicsPipeline::start()
                .vertex_input_single_buffer::<Vertex>()
                .vertex_shader(vs.main_entry_point(), ())
                .triangle_strip()
                .viewports_dynamic_scissors_irrelevant(1)
                .cull_mode_back()
                .fragment_shader(fs.main_entry_point(), ())
                .blend_alpha_blending()
                .render_pass(vulkano::framebuffer::Subpass::from(render_pass.clone(), 0).unwrap())
                .build(device.clone())
                .unwrap(),
        );

        let descriptor_sets_pool = FixedSizeDescriptorSetsPool::new(pipeline.clone() as Arc<_>, 0);

        let mut animation_images = vec![];

        let sampler = Sampler::new(
            device.clone(),
            Filter::Linear,
            Filter::Linear,
            MipmapMode::Nearest,
            SamplerAddressMode::ClampToEdge,
            SamplerAddressMode::ClampToEdge,
            SamplerAddressMode::ClampToEdge,
            // TODO: What values here
            0.0,
            1.0,
            0.0,
            0.0,
        ).unwrap();

        for image_path in &::animation::ANIMATIONS.images {
            let file = File::open(image_path).unwrap();
            let (info, mut reader) = ::png::Decoder::new(file).read_info().unwrap();
            assert_eq!(info.color_type, ::png::ColorType::RGBA);
            let mut buf = vec![0; info.buffer_size()];
            reader.next_frame(&mut buf).unwrap();

            let (image, image_fut) = ImmutableImage::from_iter(
                buf.into_iter(),
                Dimensions::Dim2d {
                    width: info.width,
                    height: info.height,
                },
                format::R8G8B8A8Srgb,
                queue.clone(),
            ).unwrap();
            future = Box::new(future.join(image_fut)) as Box<_>;

            let descriptor_set = Arc::new(
                PersistentDescriptorSet::start(pipeline.clone(), 1)
                    .add_sampled_image(image.clone(), sampler.clone())
                    .unwrap()
                    .build()
                    .unwrap(),
            ) as Arc<_>;

            animation_images.push(Image {
                descriptor_set,
                width: info.width,
                height: info.height,
            })
        }

        let view_buffer_pool =
            CpuBufferPool::<vs::ty::View>::new(device.clone(), BufferUsage::uniform_buffer());

        let world_buffer_pool =
            CpuBufferPool::<vs::ty::World>::new(device.clone(), BufferUsage::uniform_buffer());

        let depth_buffer_attachment = AttachmentImage::transient(
            device.clone(),
            images[0].dimensions(),
            format::Format::D16Unorm,
        ).unwrap();

        let framebuffers = images
            .iter()
            .map(|image| {
                Arc::new(
                    Framebuffer::start(render_pass.clone())
                        .add(image.clone())
                        .unwrap()
                        .add(depth_buffer_attachment.clone())
                        .unwrap()
                        .build()
                        .unwrap(),
                ) as Arc<_>
            })
            .collect::<Vec<_>>();

        let future = Some(Box::new(future.then_signal_fence_and_flush().unwrap()) as Box<_>);

        Graphics {
            device,
            future,
            queue,
            swapchain,
            render_pass,
            pipeline,
            vertex_buffer,
            animation_images,
            framebuffers,
            view_buffer_pool,
            world_buffer_pool,
            physical,
            descriptor_sets_pool,
        }
    }

    fn recreate(&mut self, window: &::vulkano_win::Window) {
        let recreate;
        let mut remaining_try = 20;
        loop {
            let dimensions = window
                .surface()
                .capabilities(self.physical)
                .expect("failed to get surface capabilities")
                .current_extent
                .unwrap_or([1024, 768]);

            let res = self.swapchain.recreate_with_dimension(dimensions);

            if remaining_try == 0 {
                recreate = res;
                break;
            }

            match res {
                Err(SwapchainCreationError::UnsupportedDimensions) => (),
                res @ _ => {
                    recreate = res;
                    break;
                }
            }
            remaining_try -= 1;
            ::std::thread::sleep(::std::time::Duration::from_millis(50));
        }

        let (swapchain, images) = recreate.unwrap();
        self.swapchain = swapchain;

        // TODO: factorize
        let depth_buffer_attachment = AttachmentImage::transient(
            self.device.clone(),
            images[0].dimensions(),
            format::Format::D16Unorm,
        ).unwrap();

        self.framebuffers = images
            .iter()
            .map(|image| {
                Arc::new(
                    Framebuffer::start(self.render_pass.clone())
                        .add(image.clone())
                        .unwrap()
                        .add(depth_buffer_attachment.clone())
                        .unwrap()
                        .build()
                        .unwrap(),
                ) as Arc<_>
            })
            .collect::<Vec<_>>();
    }

    fn build_command_buffer(
        &mut self,
        image_num: usize,
        world: &mut ::specs::World,
    ) -> AutoCommandBuffer<StandardCommandPoolAlloc> {
        let dimensions = self.swapchain.dimensions();

        let screen_dynamic_state = DynamicState {
            viewports: Some(vec![
                Viewport {
                    origin: [0.0, 0.0],
                    dimensions: [dimensions[0] as f32, dimensions[1] as f32],
                    depth_range: 0.0..1.0,
                },
            ]),
            ..DynamicState::none()
        };

        let mut command_buffer_builder = AutoCommandBufferBuilder::primary_one_time_submit(
            self.device.clone(),
            self.queue.family(),
        ).unwrap()
            .begin_render_pass(
                self.framebuffers[image_num].clone(),
                false,
                vec![[0.0, 0.0, 1.0, 1.0].into(), 1.0.into()],
            )
            .unwrap();

        let view = vs::ty::View {
            view: world.read_resource::<::resource::Camera>().matrix(dimensions),
        };
        let view_buffer = self.view_buffer_pool.next(view).unwrap();

        let mut images = world.write_resource::<::resource::AnimationImages>();
        for image in images.drain(..) {

            let world_matrix: ::na::Transform3<f32> = ::na::Isometry3::<f32>::new(
                ::na::Vector3::new(image.position.translation.vector[0], -image.position.translation.vector[1], 0.0),
                ::na::Vector3::new(0.0, 0.0, -image.position.rotation.angle()),
            ).to_superset();

            let world = vs::ty::World {
                world: world_matrix.unwrap().into()
            };
            let world_buffer = self.world_buffer_pool.next(world).unwrap();

            let sets = self.descriptor_sets_pool.next()
                .add_buffer(view_buffer.clone())
                .unwrap()
                .add_buffer(world_buffer)
                .unwrap()
                .build().unwrap();

            command_buffer_builder = command_buffer_builder.draw(
                self.pipeline.clone(),
                screen_dynamic_state.clone(),
                vec![self.vertex_buffer.clone()],
                (sets, self.animation_images[image.id].descriptor_set.clone()),
                vs::ty::Info {
                    layer: image.layer,
                    height: self.animation_images[image.id].height as f32,
                    width: self.animation_images[image.id].width as f32,
                },
            )
                .unwrap()
        }

        command_buffer_builder
            .end_render_pass()
            .unwrap()
            .build()
            .unwrap()
    }

    pub fn draw(&mut self, world: &mut ::specs::World, window: &::vulkano_win::Window) {
        self.future.as_mut().unwrap().cleanup_finished();

        // On X with Xmonad and intel HD graphics the acquire stay sometimes forever
        let timeout = Duration::from_secs(2);
        let mut next_image = swapchain::acquire_next_image(self.swapchain.clone(), Some(timeout));
        loop {
            match next_image {
                Err(vulkano::swapchain::AcquireError::OutOfDate)
                | Err(vulkano::swapchain::AcquireError::Timeout) => {
                    self.recreate(&window);
                    next_image =
                        swapchain::acquire_next_image(self.swapchain.clone(), Some(timeout));
                }
                _ => break,
            }
        }

        let (image_num, acquire_future) = next_image.unwrap();

        let command_buffer = self.build_command_buffer(image_num, world);

        let future = self.future
            .take()
            .unwrap()
            .join(acquire_future)
            .then_execute(self.queue.clone(), command_buffer)
            .unwrap()
            .then_swapchain_present(self.queue.clone(), self.swapchain.clone(), image_num)
            .then_signal_fence_and_flush()
            .unwrap();

        self.future = Some(Box::new(future) as Box<_>);
    }
}

mod vs {
    #[derive(VulkanoShader)]
    #[ty = "vertex"]
    #[src = "
#version 450

layout(location = 0) in vec2 position;
layout(location = 0) out vec2 tex_coords;

layout(push_constant) uniform Info {
    float layer;
    float height;
    float width;
} info;

layout(set = 0, binding = 0) uniform View {
    mat4 view;
} view;

layout(set = 0, binding = 1) uniform World {
    mat4 world;
} world;

void main() {
    gl_Position = view.view * world.world * vec4(position[0]*info.width, position[1]*info.height, info.layer, 1.0);
    tex_coords = position + vec2(0.5);
}
"]
    struct _Dummy;
}

mod fs {
    #[derive(VulkanoShader)]
    #[ty = "fragment"]
    #[src = "
#version 450

layout(location = 0) in vec2 tex_coords;
layout(location = 0) out vec4 f_color;

layout(set = 1, binding = 0) uniform sampler2D tex;

void main() {
    f_color = texture(tex, tex_coords);
}
"]
    struct _Dummy;
}

pub struct CustomRenderPassDesc {
    swapchain_image_format: Format,
}

unsafe impl RenderPassDesc for CustomRenderPassDesc {
    #[inline]
    fn num_attachments(&self) -> usize {
        2
    }

    #[inline]
    fn attachment_desc(&self, id: usize) -> Option<LayoutAttachmentDescription> {
        match id {
            // Colors
            0 => Some(LayoutAttachmentDescription {
                format: self.swapchain_image_format,
                samples: 1,
                load: LoadOp::Clear,
                store: StoreOp::Store,
                stencil_load: LoadOp::Clear,
                stencil_store: StoreOp::Store,
                initial_layout: ImageLayout::Undefined,
                final_layout: ImageLayout::ColorAttachmentOptimal,
            }),
            // Depth buffer
            1 => Some(LayoutAttachmentDescription {
                format: Format::D16Unorm,
                samples: 1,
                load: LoadOp::Clear,
                store: StoreOp::DontCare,
                stencil_load: LoadOp::Clear,
                stencil_store: StoreOp::DontCare,
                initial_layout: ImageLayout::Undefined,
                final_layout: ImageLayout::DepthStencilAttachmentOptimal,
            }),
            _ => None,
        }
    }

    #[inline]
    fn num_subpasses(&self) -> usize {
        1
    }

    #[inline]
    fn subpass_desc(&self, id: usize) -> Option<LayoutPassDescription> {
        match id {
            // draw
            0 => Some(LayoutPassDescription {
                color_attachments: vec![(0, ImageLayout::ColorAttachmentOptimal)],
                depth_stencil: Some((1, ImageLayout::DepthStencilAttachmentOptimal)),
                input_attachments: vec![],
                resolve_attachments: vec![],
                preserve_attachments: vec![],
            }),
            _ => None,
        }
    }

    #[inline]
    fn num_dependencies(&self) -> usize {
        0
    }

    #[inline]
    fn dependency_desc(&self, id: usize) -> Option<LayoutPassDependencyDescription> {
        match id {
            _ => None,
        }
    }
}

unsafe impl RenderPassDescClearValues<Vec<ClearValue>> for CustomRenderPassDesc {
    fn convert_clear_values(&self, values: Vec<ClearValue>) -> Box<Iterator<Item = ClearValue>> {
        // FIXME: safety checks
        Box::new(values.into_iter())
    }
}
