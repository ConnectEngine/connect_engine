use asset_importer::{Matrix4x4, node::Node};
use connect_shared::*;
use image::{EncodableLayout, ImageReader};
use ktx2_rw::Ktx2Texture;
use nameof::name_of;
use std::{collections::HashMap, ffi::c_void, io::Cursor, str::FromStr};
use vulkanite::vk::{
    BufferCopy, BufferUsageFlags, DeviceAddress, Extent3D, Format, ImageUsageFlags,
};

use bevy_ecs::{
    observer::On,
    system::{Commands, Res, ResMut},
};
use connect_math::{Mat4, Vec2, Vec3, Vec4};
use meshopt::{
    VertexDataAdapter, build_meshlets, optimize_vertex_cache_in_place, optimize_vertex_fetch,
    optimize_vertex_fetch_remap, remap_index_buffer, remap_vertex_buffer, typed_to_bytes,
};

use crate::engine::{
    components::local_transform::LocalTransform,
    events::{LoadModelEvent, SpawnEvent, SpawnEventRecord},
};

use connect_renderer::*;

struct NodeData {
    pub name: String,
    pub index: usize,
    pub parent_index: Option<usize>,
    pub matrix: Mat4,
    pub mesh_indices: Vec<usize>,
}

impl NodeData {
    pub fn new(
        name: String,
        index: usize,
        parent_index: Option<usize>,
        transformation: Matrix4x4,
        mesh_indices: Vec<usize>,
    ) -> Self {
        let matrix = Self::get_matrix(transformation);

        Self {
            name,
            index,
            parent_index,
            matrix,
            mesh_indices,
        }
    }

    pub fn get_matrix(transformation: Matrix4x4) -> Mat4 {
        connect_math::Mat4 {
            x_axis: Vec4::new(
                transformation.x_axis.x,
                transformation.x_axis.y,
                transformation.x_axis.z,
                transformation.x_axis.w,
            ),
            y_axis: Vec4::new(
                transformation.y_axis.x,
                transformation.y_axis.y,
                transformation.y_axis.z,
                transformation.y_axis.w,
            ),
            z_axis: Vec4::new(
                transformation.z_axis.x,
                transformation.z_axis.y,
                transformation.z_axis.z,
                transformation.z_axis.w,
            ),
            w_axis: Vec4::new(
                transformation.w_axis.x,
                transformation.w_axis.y,
                transformation.w_axis.z,
                transformation.w_axis.w,
            ),
        }
    }
}

pub fn on_load_model_system(
    load_model_event: On<LoadModelEvent>,
    mut commands: Commands,
    vulkan_context: Res<VulkanContextResource>,
    mut materials_pool: ResMut<MaterialsPoolResource>,
    renderer_context_resource: Res<RendererContextResource>,
    renderer_resources: ResMut<RendererResources>,
    mut descriptor_set_handle: ResMut<DescriptorSetHandle>,
    mut buffers_pool: ResMut<BuffersPoolResource>,
    mut textures_pool: ResMut<TexturesPoolResource>,
    mut mesh_buffers_pool: ResMut<MeshBuffersPool>,
) {
    let model_loader = &renderer_resources.model_loader;

    let mut nodes = Vec::new();

    let scene = model_loader.load_model(load_model_event.path.as_os_str().to_str().unwrap());

    let root_node_index = Default::default();
    let root_node = scene.root_node().unwrap();

    nodes.push(NodeData::new(
        root_node.name(),
        root_node_index,
        None,
        root_node.transformation(),
        get_mesh_indices(&root_node, root_node.num_meshes()),
    ));

    let mut stack: Vec<(Node, usize)> = Vec::new();
    stack.push((root_node, root_node_index));

    loop {
        while let Some((parent_node, parent_index_in_array)) = stack.pop() {
            for child_index in (0..parent_node.num_children()).rev() {
                let child_node = parent_node.child(child_index).unwrap();

                let child_index_in_array = nodes.len();
                stack.push((child_node.clone(), child_index_in_array));

                nodes.push(NodeData::new(
                    child_node.name(),
                    child_index_in_array,
                    Some(parent_index_in_array),
                    child_node.transformation(),
                    get_mesh_indices(&child_node, child_node.num_meshes()),
                ));
            }
        }

        if stack.len() == Default::default() {
            break;
        }
    }

    let mut spawn_event = SpawnEvent::default();
    spawn_event.parent_entity = load_model_event.parent_entity;
    let mut spawn_event_record = SpawnEventRecord::default();

    nodes.iter().for_each(|node_data| {
        let local_matrix = node_data.matrix;

        let (local_scale, rotation, position) = local_matrix.to_scale_rotation_translation();
        let transform = LocalTransform {
            local_position: position,
            local_rotation: rotation,
            local_scale,
        };
        spawn_event_record.name = node_data.name.clone();
        spawn_event_record.parent_index = node_data.parent_index;
        spawn_event_record.transform = transform;

        spawn_event.spawn_records.push(spawn_event_record.clone());
    });

    let mut mesh_buffers_to_upload = Vec::with_capacity(scene.num_meshes());
    let mut uploaded_mesh_buffers = HashMap::with_capacity(scene.num_meshes());
    let mut uploaded_textures = HashMap::with_capacity(uploaded_mesh_buffers.capacity());
    let mut uploaded_materials = HashMap::with_capacity(scene.num_materials());

    materials_pool.reset_materails_to_write();
    for node_data in nodes.into_iter() {
        if node_data.mesh_indices.len() > Default::default() {
            let mut mesh_name: String;
            let mut mesh_buffer_reference: MeshBufferReference;
            let mut texture_reference: TextureReference;
            for &mesh_index in node_data.mesh_indices.iter() {
                texture_reference = renderer_resources.fallback_texture_reference;
                let mesh = scene.mesh(mesh_index).unwrap();

                let material_index = mesh.material_index();
                let material_reference: MaterialReference;
                if let std::collections::hash_map::Entry::Vacant(e) =
                    uploaded_materials.entry(material_index)
                {
                    let material = scene.material(material_index).unwrap();

                    let alpha_mode = std::str::from_utf8(
                        material
                            .get_property_raw_ref(c"$mat.gltf.alphaMode", None, 0)
                            .unwrap(),
                    )
                    .unwrap();
                    let mut material_type = MaterialType::Opaque;
                    if alpha_mode.contains("BLEND") {
                        material_type = MaterialType::Transparent;
                    }

                    try_upload_texture(
                        &vulkan_context,
                        &renderer_context_resource,
                        &mut textures_pool,
                        &mut buffers_pool,
                        &mut descriptor_set_handle,
                        &scene,
                        &mut uploaded_textures,
                        material.clone(),
                        &mut texture_reference,
                        load_model_event.path.file_stem().unwrap().to_str().unwrap(),
                    );

                    let base_color_raw = material.base_color().unwrap();
                    let base_color = Vec4::new(
                        base_color_raw.x,
                        base_color_raw.y,
                        base_color_raw.z,
                        base_color_raw.w,
                    );

                    let metallic_value = material.metallic_factor().unwrap_or(0.0);
                    let roughness_value = material.roughness_factor().unwrap_or(0.0);
                    let albedo_texture_index = texture_reference.get_index();
                    let metallic_texture_index =
                        renderer_resources.fallback_texture_reference.get_index();
                    let roughness_texture_index =
                        renderer_resources.fallback_texture_reference.get_index();

                    let material_data = MaterialData {
                        material_properties: MaterialProperties::new(
                            base_color,
                            metallic_value,
                            roughness_value,
                        ),
                        material_textures: MaterialTextures::new(
                            albedo_texture_index,
                            metallic_texture_index,
                            roughness_texture_index,
                        ),
                        sampler_index: Default::default(),
                    };

                    material_reference = materials_pool.write_material(
                        bytemuck::bytes_of(&material_data),
                        MaterialState { material_type },
                    );
                    e.insert(material_reference);
                } else {
                    material_reference = *uploaded_materials.get(&material_index).unwrap();
                }

                if let std::collections::hash_map::Entry::Vacant(e) =
                    uploaded_mesh_buffers.entry(mesh_index)
                {
                    let mesh = scene.mesh(mesh_index).unwrap();
                    mesh_name = mesh.name();

                    let mut indices = Vec::with_capacity(mesh.faces().len() * 3);

                    for face in mesh.faces() {
                        for index in face.indices() {
                            indices.push(*index);
                        }
                    }

                    let positions: Vec<Vec3> = mesh
                        .vertices_iter()
                        .map(|v| Vec3::new(v.x, v.y, v.z))
                        .collect();
                    let colors: Vec<Vec3> = mesh
                        .vertex_colors(Default::default())
                        .map(|colors| {
                            colors
                                .iter()
                                .map(|color| Vec3::new(color.x, color.y, color.z))
                                .collect()
                        })
                        .unwrap_or_else(|| vec![Vec3::ZERO; positions.len()]);
                    let normals: Vec<Vec3> = mesh
                        .normals()
                        .map(|ns| ns.iter().map(|n| Vec3::new(n.x, n.y, n.z)).collect())
                        .unwrap_or_else(|| vec![Vec3::ZERO; positions.len()]);

                    let uvs: Vec<Vec2> = if mesh.has_texture_coords(0) {
                        mesh.texture_coords_iter(0)
                            .map(|uv| Vec2::new(uv.x, uv.y))
                            .collect()
                    } else {
                        vec![Vec2::ZERO; positions.len()]
                    };

                    let mut vertices = Vec::with_capacity(positions.len());
                    for i in 0..positions.len() {
                        vertices.push(Vertex {
                            position: positions[i].to_array(),
                            normal: normals[i].to_array(),
                            uv: uvs[i].to_array(),
                            color: colors[i].to_array(),
                            ..Default::default()
                        });
                    }

                    let remap = optimize_vertex_fetch_remap(&indices, vertices.len());
                    indices = remap_index_buffer(Some(&indices), vertices.len(), &remap);
                    vertices = remap_vertex_buffer(&vertices, vertices.len(), &remap);

                    let position_offset = std::mem::offset_of!(Vertex, position);
                    let vertex_stride = std::mem::size_of::<Vertex>();
                    let vertex_data = typed_to_bytes(&vertices);

                    let vertex_data_adapter =
                        VertexDataAdapter::new(vertex_data, vertex_stride, position_offset)
                            .unwrap();

                    optimize_vertex_cache_in_place(&mut indices, vertices.len());
                    let vertices = optimize_vertex_fetch(&mut indices, &vertices);

                    let (meshlets, vertex_indices, triangles) =
                        generate_meshlets(&indices, &vertex_data_adapter);

                    let vertex_buffer_reference = create_and_copy_to_buffer(
                        &mut buffers_pool,
                        vertices.as_ptr() as *const _,
                        vertices.len() * std::mem::size_of::<Vertex>(),
                        std::format!("{}_{}", mesh_name, name_of!(vertices)),
                    );
                    let vertex_indices_buffer_reference = create_and_copy_to_buffer(
                        &mut buffers_pool,
                        vertex_indices.as_ptr() as _,
                        vertex_indices.len() * std::mem::size_of::<u32>(),
                        std::format!("{}_{}", mesh_name, name_of!(vertex_indices)),
                    );
                    let meshlets_buffer_reference = create_and_copy_to_buffer(
                        &mut buffers_pool,
                        meshlets.as_ptr() as _,
                        meshlets.len() * std::mem::size_of::<Meshlet>(),
                        std::format!("{}_{}", mesh_name, name_of!(meshlets)),
                    );

                    let local_indices_buffer_reference = create_and_copy_to_buffer(
                        &mut buffers_pool,
                        triangles.as_ptr() as _,
                        triangles.len() * std::mem::size_of::<u8>(),
                        std::format!("{}_{}", mesh_name, name_of!(triangles)),
                    );

                    let mesh_data = MeshData { vertices, indices };

                    let mesh_buffer = MeshBuffer {
                        mesh_object_device_address: Default::default(),
                        vertex_buffer_reference,
                        vertex_indices_buffer_reference,
                        meshlets_buffer_reference,
                        local_indices_buffer_reference,
                        meshlets_count: meshlets.len(),
                        mesh_data,
                    };

                    mesh_buffer_reference = mesh_buffers_pool.insert_mesh_buffer(mesh_buffer);
                    mesh_buffers_to_upload.push(mesh_buffer_reference);

                    e.insert((mesh, mesh_buffer_reference));
                } else {
                    let already_uploaded_mesh = uploaded_mesh_buffers.get(&mesh_index).unwrap();
                    mesh_name = already_uploaded_mesh.0.name();
                    mesh_buffer_reference = already_uploaded_mesh.1;
                }

                spawn_event_record.name = mesh_name;
                spawn_event_record.parent_index = Some(node_data.index);
                spawn_event_record.material_reference = Some(material_reference);
                spawn_event_record.mesh_buffer_reference = Some(mesh_buffer_reference);
                spawn_event_record.transform = LocalTransform::IDENTITY;

                spawn_event.spawn_records.push(spawn_event_record.clone());
            }
        }
    }

    let mesh_objects_to_write = mesh_buffers_to_upload
        .iter()
        .map(|mesh_buffer_reference| {
            let mesh_buffer_ref = unsafe {
                mesh_buffers_pool
                    .get_mesh_buffer(*mesh_buffer_reference)
                    .unwrap_unchecked()
            };

            let device_address_vertex_buffer: DeviceAddress = mesh_buffer_ref
                .vertex_buffer_reference
                .get_buffer_info()
                .device_address;
            let device_address_vertex_indices_buffer: DeviceAddress = mesh_buffer_ref
                .vertex_indices_buffer_reference
                .get_buffer_info()
                .device_address;
            let device_address_meshlets_buffer: DeviceAddress = mesh_buffer_ref
                .meshlets_buffer_reference
                .get_buffer_info()
                .device_address;
            let device_address_local_indices_buffer: DeviceAddress = mesh_buffer_ref
                .local_indices_buffer_reference
                .get_buffer_info()
                .device_address;

            MeshObject {
                device_address_vertex_buffer,
                device_address_vertex_indices_buffer,
                device_address_meshlets_buffer,
                device_address_local_indices_buffer,
                ..Default::default()
            }
        })
        .collect::<Vec<_>>();

    let mesh_object_size = std::mem::size_of::<MeshObject>();
    let mesh_objects_device_address = renderer_resources
        .mesh_objects_buffer_reference
        .get_buffer_info()
        .device_address;

    let mesh_objects_to_copy_regions = mesh_buffers_to_upload
        .into_iter()
        .enumerate()
        .map(|(src_mesh_buffer_index, mesh_buffer_reference)| {
            let src_offset = src_mesh_buffer_index as u32 * mesh_object_size as u32;
            let dst_offset = mesh_buffer_reference.get_index() * mesh_object_size as u32;

            let mesh_buffer = unsafe {
                mesh_buffers_pool
                    .get_mesh_buffer_mut(mesh_buffer_reference)
                    .unwrap_unchecked()
            };

            mesh_buffer.mesh_object_device_address =
                mesh_objects_device_address + dst_offset as u64;

            BufferCopy {
                src_offset: src_offset as _,
                dst_offset: dst_offset as _,
                size: mesh_object_size as _,
            }
        })
        .collect::<Vec<BufferCopy>>();

    unsafe {
        buffers_pool.transfer_data_to_buffer_with_offset(
            renderer_resources.mesh_objects_buffer_reference,
            mesh_objects_to_write.as_ptr() as *const _,
            &mesh_objects_to_copy_regions,
        );
    }

    let materials_data_buffer_reference = renderer_resources.materials_data_buffer_reference;
    let materials_data_to_write_slice = materials_pool.get_materials_data_to_write();
    for (&material_reference, data_to_write) in materials_data_to_write_slice {
        let ptr_materials_data_to_write = data_to_write.as_slice().as_ptr();

        let material_instance = materials_pool
            .get_material_instance(material_reference)
            .unwrap();

        let regions = [BufferCopy {
            dst_offset: material_instance.get_offset() as _,
            size: material_instance.get_size() as _,
            ..Default::default()
        }];

        unsafe {
            buffers_pool.transfer_data_to_buffer_with_offset(
                materials_data_buffer_reference,
                ptr_materials_data_to_write as *const _,
                &regions,
            );
        }
    }

    commands.trigger(spawn_event);
}

pub fn create_and_copy_to_buffer(
    buffers_pool: &mut BuffersPoolResource,
    src: *const c_void,
    size: usize,
    name: String,
) -> BufferReference {
    let buffer_reference = buffers_pool.create_buffer(
        size,
        BufferUsageFlags::TransferDst,
        BufferVisibility::DeviceOnly,
        None,
        Some(name),
    );

    unsafe {
        buffers_pool.transfer_data_to_buffer_raw(buffer_reference, src, size);
    }

    buffer_reference
}

fn try_upload_texture(
    vulkan_context: &VulkanContextResource,
    renderer_context: &RendererContextResource,
    textures_pool: &mut TexturesPoolResource,
    buffers_pool: &mut BuffersPoolResource,
    descriptor_set_handle: &mut DescriptorSetHandle,
    scene: &asset_importer::Scene,
    uploaded_textures: &mut HashMap<usize, TextureReference>,
    material: asset_importer::Material,
    texture_reference_to_use: &mut TextureReference,
    model_name: &str,
) {
    if material.texture_count(asset_importer::TextureType::BaseColor) > Default::default() {
        let texture_info = material
            .texture(asset_importer::TextureType::BaseColor, Default::default())
            .unwrap();
        let texture_index = texture_info.path[1..].parse::<usize>().unwrap();

        if let std::collections::hash_map::Entry::Vacant(e) = uploaded_textures.entry(texture_index)
        {
            let texture = scene.texture(texture_index).unwrap();
            let texture_name = texture
                .filename()
                .unwrap_or(std::format!("{model_name}_texture_{texture_index}"));

            let (texture_reference, texture_data) = try_to_load_cached_texture(
                textures_pool,
                model_name,
                texture.clone(),
                &texture_name,
            );
            *texture_reference_to_use = texture_reference;

            vulkan_context.transfer_data_to_image(
                textures_pool.get_image(texture_reference).unwrap(),
                buffers_pool,
                texture_data.as_ptr() as *const _,
                &renderer_context.upload_context,
                Some(texture_data.len()),
            );

            let descriptor_texture = DescriptorKind::SampledImage(DescriptorSampledImage {
                image_view: textures_pool
                    .get_image(texture_reference)
                    .unwrap()
                    .image_view,
                index: texture_reference.get_index(),
            });
            descriptor_set_handle.update_binding(buffers_pool, descriptor_texture);

            let texture_metadata = texture_reference.texture_metadata;
            println!(
                "Name: {} | Index: {} | Extent: {}x{}x{}",
                texture_name,
                texture_reference.get_index(),
                texture_metadata.width,
                texture_metadata.height,
                1,
            );

            e.insert(texture_reference);
        } else {
            *texture_reference_to_use = *uploaded_textures.get(&texture_index).unwrap();
        }
    }
}

fn try_to_load_cached_texture(
    textures_pool: &mut TexturesPoolResource,
    model_name: &str,
    texture: asset_importer::Texture,
    texture_name: &str,
) -> (TextureReference, Vec<u8>) {
    let mut path = std::path::PathBuf::from("intermediate/textures/");
    path.push(model_name);
    std::fs::create_dir_all(&path).unwrap();

    path.push(String::from_str(texture_name).unwrap());
    let does_exist = std::fs::exists(&path).unwrap();

    let texture_reference: TextureReference;
    let mut texture_data: Vec<u8> = Vec::new();

    if does_exist {
        let texture = Ktx2Texture::from_file(&path).unwrap();
        let texture_metadata_raw: Vec<u8> =
            texture.get_metadata(stringify!(TextureMetadata)).unwrap();
        println!(
            "Path: {} | De Texture Metadata Length: {}",
            path.display(),
            texture_metadata_raw.len()
        );

        let texture_metadata =
            rkyv::from_bytes::<TextureMetadata, rkyv::rancor::Error>(&texture_metadata_raw)
                .unwrap();

        for mip_level_index in 0..texture_metadata.mip_levels_count {
            texture_data.extend_from_slice(texture.get_image_data(mip_level_index, 0, 0).unwrap());
        }

        let extent = Extent3D {
            width: texture_metadata.width,
            height: texture_metadata.height,
            depth: 1,
        };

        let (created_texture_reference, _) = textures_pool.create_texture(
            Some(&mut texture_data),
            true,
            Format::Bc1RgbSrgbBlock,
            extent,
            ImageUsageFlags::Sampled | ImageUsageFlags::TransferDst,
            true,
        );

        texture_reference = created_texture_reference;
    } else {
        let mut data = texture.data_bytes_ref().unwrap();

        let cursor = Cursor::new(&mut data);

        let image = ImageReader::new(cursor)
            .with_guessed_format()
            .unwrap()
            .decode()
            .unwrap();

        let extent = Extent3D {
            width: image.width(),
            height: image.height(),
            depth: 1,
        };
        let rgba_image = image.to_rgba8();
        let mut image_bytes = rgba_image.as_bytes().to_vec();

        let (created_texture_reference, ktx_texture) = textures_pool.create_texture(
            Some(&mut image_bytes),
            false,
            Format::Bc1RgbSrgbBlock,
            extent,
            ImageUsageFlags::Sampled | ImageUsageFlags::TransferDst,
            true,
        );
        texture_reference = created_texture_reference;

        let ktx_texture = ktx_texture.unwrap();
        for mip_level_index in 0..created_texture_reference.texture_metadata.mip_levels_count {
            texture_data
                .extend_from_slice(ktx_texture.get_image_data(mip_level_index, 0, 0).unwrap());
        }

        ktx_texture.write_to_file(path).unwrap();
    }

    (texture_reference, texture_data)
}

fn get_mesh_indices(node: &Node, num_meshes: usize) -> Vec<usize> {
    let mut mesh_indices = Vec::with_capacity(num_meshes);
    if num_meshes > Default::default() {
        for mesh_index in node.mesh_indices() {
            mesh_indices.push(mesh_index);
        }
    }

    mesh_indices
}

fn generate_meshlets(
    indices: &[u32],
    vertices: &VertexDataAdapter,
) -> (Vec<Meshlet>, Vec<u32>, Vec<u8>) {
    let max_vertices = 64;
    let max_triangles = 64;
    let cone_weight = 0.0;

    let raw_meshlets = build_meshlets(indices, vertices, max_vertices, max_triangles, cone_weight);

    let mut meshlets = Vec::new();

    for raw_meshlet in raw_meshlets.meshlets.iter() {
        meshlets.push(Meshlet {
            vertex_offset: raw_meshlet.vertex_offset as _,
            triangle_offset: raw_meshlet.triangle_offset as _,
            vertex_count: raw_meshlet.vertex_count as _,
            triangle_count: raw_meshlet.triangle_count as _,
            ..Default::default()
        });
    }

    (meshlets, raw_meshlets.vertices, raw_meshlets.triangles)
}
