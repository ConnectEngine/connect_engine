import modules.resources;

#ifdef MATERIAL_TYPE
typealias Material = MATERIAL_TYPE;
#else
typealias Material = DefaultMaterial;
#endif

struct PrimitiveData
{
    nointerpolation const let device_address_material : ImmutablePtr<Material>;
}

enum MaterialType : uint8_t
{
    Opaque,
    Transparent,
}

struct Vertex
{
    let position : float3;
    let normal : float3;
    let uv : float2;
    let color : float3;
};

struct VertexOutput
{
    float4 position : SV_Position;
    var normal : float3;
    var uv : float2;
    var color : float3;
    var world_position : float3;
};

struct Meshlet
{
    const let vertex_offset : uint32_t;
    const let triangle_offset : uint32_t;
    const let vertex_count : uint32_t;
    const let triangle_count : uint32_t;
}

struct MeshObject
{
    const let vertices : ImmutablePtr<Vertex>;
    const let vertex_indices : ImmutablePtr<uint32_t>;
    const let meshlets : ImmutablePtr<Meshlet>;
    const let local_indices : ImmutablePtr<uint8_t>;
}

struct InstanceObject
{
    const let model_matrix : float4x4;
    const let normal_matrix : float4x4;
    const let ptr_mesh_object : ImmutablePtr<MeshObject>;
    const let device_address_material : ImmutablePtr<Material>;
    const let meshlet_count : uint32_t;
    const let material_type : MaterialType;
}

struct Payload
{
    const let instance_object_index : uint32_t;
}

struct SurfaceData
{
    var color : float4 = float4(1.0);
    var metallic : float32_t = 0.0;
    var roughness : float32_t = 0.0;
}

interface IMaterial
{
    func eval(surface_data: SurfaceData, const uv: float2)->SurfaceData;
}

struct MaterialProperties
{
    let base_color : float4;
    let metallic_value : float32_t;
    let roughness_value : float32_t;
}

struct MaterialTextures
{
    let albedo_texture_index : uint32_t;
    let metallic_texture_index : uint32_t;
    let roughness_texture_index : uint32_t;
}

struct UnlitMaterial : IMaterial
{
    let material_properties : MaterialProperties;
    let material_textures : MaterialTextures;
    let sampler_index : uint32_t;

    [ForceInline]
    func eval(SurfaceData surface_data, const uv: float2)->SurfaceData
    {
        const let sampler = samplers[sampler_index];
        const let texture = sampled_images[material_textures.albedo_texture_index];

        var color = texture.Sample(sampler, uv);
        color *= material_properties.base_color;

        surface_data.color = color;
        surface_data.metallic = material_properties.metallic_value;
        surface_data.roughness = material_properties.roughness_value;

        return surface_data;
    }
}

struct DefaultMaterial : IMaterial
{
    func eval(SurfaceData surface_data, const uv: float2)->SurfaceData
    {
        surface_data.color = float4(1.0);

        return surface_data;
    }
}

struct LightProperties
{
    let ambient_color : float4;
    let ambient_strength : float32_t;
    let specular_strength : float32_t;
    let _padding : float2;
}

struct DirectionalLight
{
    let light_color : float3;
    let light_position : float3;
    let _padding : float2;
}

struct SceneData
{
    let camera_view_matrix : float4x4;
    let camera_position : float3;
    let _padding : float32_t;
    let light_properties : LightProperties;
    let directional_light : DirectionalLight;
}

struct GlobalPushConstants
{
    const let ptr_scene_data : ImmutablePtr<SceneData>;
    const let ptr_instance_object : ImmutablePtr<InstanceObject>;
    const let draw_image_index : uint32_t;
    const let current_material_type : MaterialType;
};

[[vk::push_constant]]
ConstantBuffer<GlobalPushConstants> push_constants;
