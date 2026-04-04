import modules;

static const let MAX_VERTICES : uint32_t = 64;
static const let MAX_TRIANGLES : uint32_t = 64;
static const let GROUP_SIZE : uint32_t = MAX_VERTICES;

groupshared Payload payload;

///////////////////////////////////////////////////// TASK //////////////////////////////////////////////////////////////

[shader("amplification")]
[numthreads(1, 1, 1)]
func main(const uint32_t group_id: SV_GroupID)
{
    const let instance_object = push_constants.ptr_instance_object[group_id];

    let is_current_material_type = push_constants.current_material_type == instance_object.material_type;
    let is_current_material_type_mask = (uint32_t)is_current_material_type;

    payload = Payload(group_id);

    DispatchMesh(instance_object.meshlet_count * is_current_material_type_mask, 1, 1, payload);
}

///////////////////////////////////////////////////// MESH //////////////////////////////////////////////////////////////

[shader("mesh")]
[outputtopology("triangle")]
[numthreads(GROUP_SIZE, 1, 1)]
func main(
    const uint group_id: SV_GroupID,
    const uint group_index: SV_GroupIndex,
    OutputVertices<VertexOutput, MAX_VERTICES> outVerts,
    OutputIndices<uint3, MAX_TRIANGLES> outIndices,
    OutputPrimitives<PrimitiveData, MAX_TRIANGLES> out_primitives)
{
    const let instance_object = push_constants.ptr_instance_object[payload.instance_object_index];
    let ptr_mesh_object = instance_object.ptr_mesh_object;
    let meshlet = ptr_mesh_object.meshlets[group_id];

    SetMeshOutputCounts(meshlet.vertex_count, meshlet.triangle_count);

    if (group_index < meshlet.vertex_count)
    {
        let vertexLookupIndex = meshlet.vertex_offset + group_index;
        let globalVertexId = ptr_mesh_object.vertex_indices[vertexLookupIndex];

        let v = ptr_mesh_object.vertices[globalVertexId];
        outVerts[group_index] = process_vertex(v,
                                               instance_object.model_matrix,
                                               instance_object.normal_matrix,
                                               push_constants.ptr_scene_data.camera_view_matrix);
    }

    if (group_index < meshlet.triangle_count)
    {
        let triangleLookupIndex = meshlet.triangle_offset + (group_index * 3);
        let i0 = ptr_mesh_object.local_indices[triangleLookupIndex + 0];
        let i1 = ptr_mesh_object.local_indices[triangleLookupIndex + 1];
        let i2 = ptr_mesh_object.local_indices[triangleLookupIndex + 2];

        outIndices[group_index] = uint32_t3(i0, i1, i2);

        out_primitives[group_index] = PrimitiveData(instance_object.device_address_material);
    }
}

[ForceInline]
func process_vertex(const v: Vertex,
                    const model_matrix: float4x4,
                    const normal_matrix: float4x4,
                    const view_projection_matrix: float4x4)
    ->VertexOutput
{
    let position = float4(v.position, 1.0);
    let world_position = mul(model_matrix, position);
    let clip_position = mul(view_projection_matrix, world_position);

    let N_matrix = float3x3(normal_matrix[0].xyz, normal_matrix[1].xyz, normal_matrix[2].xyz);
    let normal = mul(N_matrix, v.normal).xyz;

    let vertex_output : VertexOutput = VertexOutput(clip_position, normal, v.uv, v.color, world_position.xyz);

    return vertex_output;
}

///////////////////////////////////////////////////// FRAGMENT //////////////////////////////////////////////////////////////

[shader("fragment")]
func main(const vertex_output: VertexOutput, const primitive_data: PrimitiveData)->float4
{
    let scene_data = push_constants.ptr_scene_data;
    let material = primitive_data.device_address_material;
    var surface_data = material.eval(SurfaceData(), vertex_output.uv);

    let light_properties = scene_data.light_properties;
    let directional_light = scene_data.directional_light;

    let N = normalize(vertex_output.normal);
    let V = normalize(scene_data.camera_position - vertex_output.world_position);
    let L = normalize(-directional_light.light_direction);

    let radiance = directional_light.light_color * directional_light.intensity;

    var color = brdf(surface_data.color.rgb, N, V, L, surface_data.metallic, max(surface_data.roughness, 0.045), radiance);

    let ambient = surface_data.color.rgb * light_properties.ambient_color.rgb * light_properties.ambient_strength;
    color += ambient;

    color = color / (color + float3(1.0));
    color = pow(color, float3(1.0 / 2.2));

    return float4(color, surface_data.color.a);
}

// NOTE: POINT LIGHT
/* [shader("fragment")]
func main(const vertex_output: VertexOutput, const primitive_data: PrimitiveData)->float4
{
    let scene_data = push_constants.ptr_scene_data;
    let material = primitive_data.device_address_material;
    var surface_data = material.eval(SurfaceData(), vertex_output.uv);

    let light_properties = scene_data.light_properties;
    let point_light = scene_data.point_light;

    let L_unnormalized = point_light.light_position - vertex_output.world_position;
    let distance = length(L_unnormalized);

    let distance_in_meters = distance / 100000.0;
    let distance_squared = dot(L_unnormalized, L_unnormalized);

    let N = normalize(vertex_output.normal);
    let V = normalize(scene_data.camera_position - vertex_output.world_position);
    let L = L_unnormalized / distance;

    let luminous_intensity = point_light.intensity / (4.0 * float32_t.getPi());

    let influence_radius_squared = point_light.influence_radius * point_light.influence_radius;
    let distance_ratio_squared = distance_squared / max(influence_radius_squared, 0.0001);
    let distance_ratio_quad = distance_ratio_squared * distance_ratio_squared;

    let window_function = saturate(1.0 - distance_ratio_quad);
    let window_function_squared = window_function * window_function;

    let source_radius_squared = point_light.source_radius * point_light.source_radius;
    let inverse_square_falloff = 1.0 / max((distance_squared + source_radius_squared), 0.0001);

    var attenuation = window_function_squared * inverse_square_falloff;

    let radiance = point_light.light_color * luminous_intensity * attenuation;

    var color = brdf(surface_data.color.rgb, N, V, L, surface_data.metallic, max(surface_data.roughness, 0.045), radiance);

    attenuation = 1.0 / max((distance * distance), 0.0001);
    color *= attenuation;

    let ambient = surface_data.color.rgb * light_properties.ambient_color.rgb * light_properties.ambient_strength;
    color += ambient;

    color = color / (color + float3(1.0));
    color = pow(color, float3(1.0 / 2.2));

    return float4(color, surface_data.color.a);
} */

[ForceInline]
func brdf(const albedo: float3,
          const N: float3,
          const V: float3,
          const L: float3,
          const metallic: float32_t,
          const roughness: float32_t,
          const radiance: float3)
    ->float3
{
    let H = normalize(L + V);

    let NoV = abs(dot(N, V)) + 1e-5;
    let NoL = saturate(dot(N, L));
    let NoH = saturate(dot(N, H));
    let LoH = saturate(dot(L, H));
    let LoN = saturate(dot(L, N));
    let VoN = saturate(dot(V, N));

    let D = d_ggx(NoH, roughness * roughness);

    let f0 = 0.16 * 0.5 * 0.5 * (1.0 - metallic) + albedo * metallic;
    let F = f_shlick(LoH, f0);

    let G = g_smith(NoV, NoL, roughness * roughness);

    let Fr = (D * G) * F;
    let Fd = fd_burley(albedo, roughness, LoH, LoN, VoN);

    let kS = F;
    let kD = (1.0 - kS) * (1.0 - metallic);

    let color = (kD * Fd + Fr) * radiance * NoL;

    return color;
}

[ForceInline]
func fd_burley(base_color: float3, a: float32_t, LoH: float32_t, LoN: float32_t, VoN: float32_t)->float3
{
    let F90 = 0.5 + 2 * a * (LoH * LoH);
    return base_color * 1.0 / float32_t.getPi() * (1.0 + (F90 - 1.0) * pow(1 - LoN, 5.0)) * (1 + (F90 - 1.0) * pow(1.0 - VoN, 5.0));
}

[ForceInline]
func d_ggx(const NoH: float32_t, const a: float32_t)->float32_t
{
    let a2 = a * a;
    let f = (NoH * a2 - NoH) * NoH + 1.0;

    return a2 / (float32_t.getPi() * f * f);
}

[ForceInline]
func g_smith(const NoV: float32_t, const NoL: float32_t, const a: float32_t)->float32_t
{
    let a2 = a * a;

    let GGXL = NoV * sqrt((-NoL * a2 + NoL) * NoL + a2);
    let GGXV = NoL * sqrt((-NoV * a2 + NoV) * NoV + a2);

    return 0.5 / (GGXV + GGXL);
}

[ForceInline]
func f_shlick(const u: float32_t, const f0: float3)->float3
{
    return f0 + (float3(1.0) - f0) * pow(1.0 - u, 5.0);
}

float3 get_camera_position_from_view(float4x4 viewMatrix)
{
    float3x3 R = float3x3(
        viewMatrix[0].xyz,
        viewMatrix[1].xyz,
        viewMatrix[2].xyz);

    float3 T = viewMatrix[3].xyz;

    return mul(transpose(R), -T);
}
