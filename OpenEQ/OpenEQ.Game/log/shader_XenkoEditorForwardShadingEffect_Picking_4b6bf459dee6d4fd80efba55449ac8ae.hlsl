/**************************
***** Compiler Parameters *****
***************************
@P EffectName: XenkoEditorForwardShadingEffect.Picking
@P   - Material.PixelStageSurfaceShaders: mixin MaterialSurfaceArray [{layers = [mixin MaterialSurfaceDiffuse [{diffuseMap = ComputeColorTextureScaledOffsetDynamicSampler<Material.DiffuseMap,TEXCOORD0,Material.Sampler.i0,rgba,Material.TextureScale,Material.TextureOffset>}], mixin MaterialSurfaceLightingAndShading [{surfaces = [MaterialSurfaceShadingDiffuseLambert<false>]}]]}]
@P Material.PixelStageStreamInitializer: mixin MaterialStream, MaterialPixelShadingStream
@P Lighting.DirectLightGroups: LightClusteredPointGroup<64>, LightClusteredSpotGroup<64>, mixin LightDirectionalGroup<8>, mixin LightDirectionalGroup<1>, ShadowMapReceiverDirectional<4,1,true,true,false>, ShadowMapFilterDefault<PerView.Lighting>
@P Lighting.EnvironmentLights: LightSimpleAmbient, xkfx LightSkyboxEffect
@P LightSkyboxShader.LightDiffuseColor.environmentLights[1]: SphericalHarmonicsEnvironmentColor<3>
@P LightSkyboxShader.LightSpecularColor.environmentLights[1]: RoughnessCubeMapEnvironmentColor
***************************
****  ConstantBuffers  ****
***************************
cbuffer PerDraw [Size: 432]
@C    World_id29 => Transformation.World
@C    WorldInverse_id30 => Transformation.WorldInverse
@C    WorldInverseTranspose_id31 => Transformation.WorldInverseTranspose
@C    WorldView_id32 => Transformation.WorldView
@C    WorldViewInverse_id33 => Transformation.WorldViewInverse
@C    WorldViewProjection_id34 => Transformation.WorldViewProjection
@C    WorldScale_id35 => Transformation.WorldScale
@C    EyeMS_id36 => Transformation.EyeMS
@C    PickingData_id76 => PickingShader.PickingData
cbuffer PerView [Size: 1280]
@C    View_id22 => Transformation.View
@C    ViewInverse_id23 => Transformation.ViewInverse
@C    Projection_id24 => Transformation.Projection
@C    ProjectionInverse_id25 => Transformation.ProjectionInverse
@C    ViewProjection_id26 => Transformation.ViewProjection
@C    ProjScreenRay_id27 => Transformation.ProjScreenRay
@C    Eye_id28 => Transformation.Eye
@C    NearClipPlane_id78 => Camera.NearClipPlane
@C    FarClipPlane_id79 => Camera.FarClipPlane
@C    ZProjection_id80 => Camera.ZProjection
@C    ViewSize_id81 => Camera.ViewSize
@C    AspectRatio_id82 => Camera.AspectRatio
@C    ClusterDepthScale_id88 => LightClustered.ClusterDepthScale
@C    ClusterDepthBias_id89 => LightClustered.ClusterDepthBias
@C    LightCount_id95 => DirectLightGroupPerView.LightCount.directLightGroups[2]
@C    Lights_id97 => LightDirectionalGroup.Lights.directLightGroups[2]
@C    LightCount_id98 => DirectLightGroupPerView.LightCount.directLightGroups[3]
@C    Lights_id100 => LightDirectionalGroup.Lights.directLightGroups[3]
@C    ShadowMapTextureSize_id102 => ShadowMap.TextureSize.directLightGroups[3]
@C    ShadowMapTextureTexelSize_id103 => ShadowMap.TextureTexelSize.directLightGroups[3]
@C    WorldToShadowCascadeUV_id164 => ShadowMapReceiverBase.WorldToShadowCascadeUV.directLightGroups[3]
@C    DepthBiases_id165 => ShadowMapReceiverBase.DepthBiases.directLightGroups[3]
@C    OffsetScales_id166 => ShadowMapReceiverBase.OffsetScales.directLightGroups[3]
@C    CascadeDepthSplits_id172 => ShadowMapReceiverDirectional.CascadeDepthSplits.directLightGroups[3]
@C    AmbientLight_id173 => LightSimpleAmbient.AmbientLight.environmentLights[0]
@C    SphericalColors_id175 => SphericalHarmonicsEnvironmentColor.SphericalColors.lightDiffuseColor.environmentLights[1]
@C    MipCount_id176 => RoughnessCubeMapEnvironmentColor.MipCount.lightSpecularColor.environmentLights[1]
@C    SkyMatrix_id178 => LightSkyboxShader.SkyMatrix.environmentLights[1]
@C    Intensity_id179 => LightSkyboxShader.Intensity.environmentLights[1]
***************************
******  Resources    ******
***************************
@R    PerDraw => PerDraw [Stage: Vertex, Slot: (0-0)]
@R    PerView => PerView [Stage: Vertex, Slot: (1-1)]
@R    PerDraw => PerDraw [Stage: Pixel, Slot: (0-0)]
***************************
*****     Sources     *****
***************************
@S    ShaderBase => 5c3160ccbf936c5e102bd9315f0f946a
@S    ShaderBaseStream => f699080594ac63c1f38140de57ce1a7b
@S    ShadingBase => a4b46f7608ab80e77c938f475834f962
@S    ComputeColor => 6e3e8655257003c26480195de33cbbc1
@S    TransformationBase => f1386ed2f0e8db4eb9e79a63ee23e05c
@S    NormalStream => 5d295e278c920e30e310e1979bc14a19
@S    TransformationWAndVP => 1ad0d61ab4ab5bf92b87169f094b8b17
@S    PositionStream4 => 87a2d7a46c028c1479a982866d88d067
@S    PositionHStream4 => f5c4e6d128281ef2d40fba86959205d5
@S    Transformation => 8b044dd67dc836d93d27ce6bee5b1db8
@S    NormalFromMesh => cac9ac354def411344b0994c9fd56e66
@S    NormalBase => b70fd93cea3e78cb7e35bf33c5bc761d
@S    MaterialSurfacePixelStageCompositor => 1d6419ba7ac2d0f180564d21a0d3f8c0
@S    PositionStream => 664ed5997626578c1dfdc74402cd23f6
@S    MaterialPixelShadingStream => 28de6f1960d9c29f165f916c0adfe5a5
@S    MaterialPixelStream => 17c22a7a37d239e0ee9e8d2779c03c80
@S    MaterialStream => 83fd257dadb68d92b2e57a11122265d4
@S    IStreamInitializer => 8be0404675b438bff2babfd297fb96df
@S    LightStream => 81d82a322111295057787d950e438228
@S    DirectLightGroupArray => 9addcff5b7cb7364a8a5d87d4de45742
@S    DirectLightGroup => 92eb9d726b1937533c27d57ff97f8da5
@S    ShadowGroup => 545ec47456ee7f4472536d8a7316d02f
@S    ShadowStream => 4abbfb30b4b85e71b0f683b420977101
@S    EnvironmentLightArray => 92f59b0813153de143701e01f1da89e0
@S    EnvironmentLight => 461117f7455b162e23645a86e89f749d
@S    IMaterialSurface => 14b3d59b9c516893bd7730aaa3dc225a
@S    PickingShader => 5de66226307c4493dc53ba9855a2a8f8
@S    LightClusteredPointGroup => 60ed7b620bc6eabb3ad2330302bc367e
@S    LightPoint => 2044aa0ce4ba04060728e554f889fe5c
@S    LightUtil => d77cfcfacea55ef4e06d9f585be27420
@S    LightClustered => e0cc7997bb88b676801a857b727faf2b
@S    Camera => ec417735d81c9ad1d80ca0e6d87138b0
@S    LightClusteredSpotGroup => 7beef740595052a48885ae9f619304ca
@S    LightSpot => 79cce87c7d61f0fcb32b260513b6f8c2
@S    LightDirectionalGroup => 7bb2cc43889ab80004ab02a83bbd02d2
@S    DirectLightGroupPerView => f728a5a7f613dcd26bc20d54908ca1b8
@S    LightDirectional => 8f8fc60cae27222c84186af6eec74080
@S    ShadowMapReceiverDirectional => 95d60505c10a89c26292fb44b5445492
@S    ShadowMapReceiverBase => b8e884a10ba47c78372ae2ee6b0eada5
@S    ShadowMapGroup => 9fa1018d7f62b9d4719202422d304c06
@S    ShadowMapCommon => f7f2751f116d20510441e8bbc4c1bea6
@S    ShadowMapFilterBase => 4f39c8431637ddf01ca0702bbf3fa5d8
@S    Texturing => a9d9ee1fb9a68af7ce6b12291686faea
@S    ShadowMapFilterDefault => 8eeeb621ca5b11a4149e6e5e72265672
@S    LightSimpleAmbient => 6d68fc78fb5b5ad232c5ec44f6d90e31
@S    LightSkyboxShader => 346d0cba52d046c17e99ecff0bfe9ba4
@S    IComputeEnvironmentColor => 9e5e0467c0fccc46f26afa70c63aed71
@S    SphericalHarmonicsEnvironmentColor => da5f7069400fbe0cfd86d1dff821ca1a
@S    RoughnessCubeMapEnvironmentColor => c519c7467719a806c6407c9256543142
@S    MaterialSurfaceArray => ccbac4b49ee1e91b6f7d11a29c26a658
@S    MaterialSurfaceDiffuse => 4572009ce562fa4c388af46aacc800f7
@S    IMaterialSurfacePixel => c2baefbc3fb7ad1466cc087909a23051
@S    ComputeColorTextureScaledOffsetDynamicSampler => 36853d1a7bd080d8e9e9c5137a104b79
@S    DynamicTexture => 74e692ca9cc93b6cc6965bc380e07efe
@S    DynamicSampler => 0e1b689d1f965f3f34ae76abdf34dea7
@S    DynamicTextureStream => 546b0c40a42b4fe0e5613396b0ed4d66
@S    MaterialSurfaceLightingAndShading => 4faacd53863c0b5c65d23ad20797d27c
@S    Math => 5bdf39e412cac464ebbce340da34bca1
@S    IMaterialSurfaceShading => 7aecb9db43b53094f40d49ae22406a18
@S    MaterialSurfaceShadingDiffuseLambert => 66ea94572ee9f10a2e305608a7d69dad
***************************
*****     Stages      *****
***************************
@G    Vertex => 36721488cb6f14bd3c8a9c013bc3a31c
//
// Generated by Microsoft (R) HLSL Shader Compiler 6.3.9600.16384
//
//
// Buffer Definitions: 
//
// cbuffer PerDraw
// {
//
//   float4x4 World_id29;               // Offset:    0 Size:    64
//   float4x4 WorldInverse_id30;        // Offset:   64 Size:    64 [unused]
//   float4x4 WorldInverseTranspose_id31;// Offset:  128 Size:    64 [unused]
//   float4x4 WorldView_id32;           // Offset:  192 Size:    64 [unused]
//   float4x4 WorldViewInverse_id33;    // Offset:  256 Size:    64 [unused]
//   float4x4 WorldViewProjection_id34; // Offset:  320 Size:    64 [unused]
//   float3 WorldScale_id35;            // Offset:  384 Size:    12 [unused]
//   float4 EyeMS_id36;                 // Offset:  400 Size:    16 [unused]
//   float4 PickingData_id76;           // Offset:  416 Size:    16 [unused]
//
// }
//
// cbuffer PerView
// {
//
//   float4x4 View_id22;                // Offset:    0 Size:    64 [unused]
//   float4x4 ViewInverse_id23;         // Offset:   64 Size:    64 [unused]
//   float4x4 Projection_id24;          // Offset:  128 Size:    64 [unused]
//   float4x4 ProjectionInverse_id25;   // Offset:  192 Size:    64 [unused]
//   float4x4 ViewProjection_id26;      // Offset:  256 Size:    64
//   float2 ProjScreenRay_id27;         // Offset:  320 Size:     8 [unused]
//   float4 Eye_id28;                   // Offset:  336 Size:    16 [unused]
//   float NearClipPlane_id78;          // Offset:  352 Size:     4 [unused]
//      = 0x3f800000 
//   float FarClipPlane_id79;           // Offset:  356 Size:     4 [unused]
//      = 0x42c80000 
//   float2 ZProjection_id80;           // Offset:  360 Size:     8 [unused]
//   float2 ViewSize_id81;              // Offset:  368 Size:     8 [unused]
//   float AspectRatio_id82;            // Offset:  376 Size:     4 [unused]
//   float ClusterDepthScale_id88;      // Offset:  380 Size:     4 [unused]
//   float ClusterDepthBias_id89;       // Offset:  384 Size:     4 [unused]
//   int LightCount_id95;               // Offset:  388 Size:     4 [unused]
//   
//   struct DirectionalLightData
//   {
//       
//       float3 DirectionWS;            // Offset:  400
//       float3 Color;                  // Offset:  416
//
//   } Lights_id97[8];                  // Offset:  400 Size:   252 [unused]
//   int LightCount_id98;               // Offset:  652 Size:     4 [unused]
//   
//   struct DirectionalLightData
//   {
//       
//       float3 DirectionWS;            // Offset:  656
//       float3 Color;                  // Offset:  672
//
//   } Lights_id100;                    // Offset:  656 Size:    28 [unused]
//   float2 ShadowMapTextureSize_id102; // Offset:  688 Size:     8 [unused]
//   float2 ShadowMapTextureTexelSize_id103;// Offset:  696 Size:     8 [unused]
//   float4x4 WorldToShadowCascadeUV_id164[4];// Offset:  704 Size:   256 [unused]
//   float DepthBiases_id165;           // Offset:  960 Size:     4 [unused]
//   float OffsetScales_id166;          // Offset:  976 Size:     4 [unused]
//   float CascadeDepthSplits_id172[4]; // Offset:  992 Size:    52 [unused]
//   float3 AmbientLight_id173;         // Offset: 1044 Size:    12 [unused]
//   float3 SphericalColors_id175[9];   // Offset: 1056 Size:   140 [unused]
//   float MipCount_id176;              // Offset: 1196 Size:     4 [unused]
//   float4x4 SkyMatrix_id178;          // Offset: 1200 Size:    64 [unused]
//   float Intensity_id179;             // Offset: 1264 Size:     4 [unused]
//
// }
//
//
// Resource Bindings:
//
// Name                                 Type  Format         Dim Slot Elements
// ------------------------------ ---------- ------- ----------- ---- --------
// PerDraw                           cbuffer      NA          NA    0        1
// PerView                           cbuffer      NA          NA    1        1
//
//
//
// Input signature:
//
// Name                 Index   Mask Register SysValue  Format   Used
// -------------------- ----- ------ -------- -------- ------- ------
// POSITION                 0   xyzw        0     NONE   float   xyzw
// NORMAL                   0   xyz         1     NONE   float       
//
//
// Output signature:
//
// Name                 Index   Mask Register SysValue  Format   Used
// -------------------- ----- ------ -------- -------- ------- ------
// SV_Position              0   xyzw        0      POS   float   xyzw
//
vs_5_0
dcl_globalFlags refactoringAllowed
dcl_constantbuffer cb0[4], immediateIndexed
dcl_constantbuffer cb1[20], immediateIndexed
dcl_input v0.xyzw
dcl_output_siv o0.xyzw, position
dcl_temps 1
//
// Initial variable locations:
//   v0.x <- __input__.Position_id18.x; v0.y <- __input__.Position_id18.y; v0.z <- __input__.Position_id18.z; v0.w <- __input__.Position_id18.w; 
//   v1.x <- __input__.meshNormal_id14.x; v1.y <- __input__.meshNormal_id14.y; v1.z <- __input__.meshNormal_id14.z; 
//   o0.x <- <VSMain return value>.ShadingPosition_id0.x; o0.y <- <VSMain return value>.ShadingPosition_id0.y; o0.z <- <VSMain return value>.ShadingPosition_id0.z; o0.w <- <VSMain return value>.ShadingPosition_id0.w
//
#line 147 "c:\aaa\projects\OpenEQ\OpenEQ\OpenEQ.Game\log\shader_XenkoEditorForwardShadingEffect_Picking_4b6bf459dee6d4fd80efba55449ac8ae.hlsl"
dp4 r0.x, v0.xyzw, cb0[0].xyzw  // r0.x <- streams.PositionWS_id19.x
dp4 r0.y, v0.xyzw, cb0[1].xyzw  // r0.y <- streams.PositionWS_id19.y
dp4 r0.z, v0.xyzw, cb0[2].xyzw  // r0.z <- streams.PositionWS_id19.z
dp4 r0.w, v0.xyzw, cb0[3].xyzw  // r0.w <- streams.PositionWS_id19.w

#line 137
dp4 o0.x, r0.xyzw, cb1[16].xyzw
dp4 o0.y, r0.xyzw, cb1[17].xyzw
dp4 o0.z, r0.xyzw, cb1[18].xyzw
dp4 o0.w, r0.xyzw, cb1[19].xyzw

#line 184
ret 
// Approximately 9 instruction slots used
@G    Pixel => 90a0e455e1391d11f0ad267d4bb5b20d
//
// Generated by Microsoft (R) HLSL Shader Compiler 6.3.9600.16384
//
//
// Buffer Definitions: 
//
// cbuffer PerDraw
// {
//
//   float4x4 World_id29;               // Offset:    0 Size:    64 [unused]
//   float4x4 WorldInverse_id30;        // Offset:   64 Size:    64 [unused]
//   float4x4 WorldInverseTranspose_id31;// Offset:  128 Size:    64 [unused]
//   float4x4 WorldView_id32;           // Offset:  192 Size:    64 [unused]
//   float4x4 WorldViewInverse_id33;    // Offset:  256 Size:    64 [unused]
//   float4x4 WorldViewProjection_id34; // Offset:  320 Size:    64 [unused]
//   float3 WorldScale_id35;            // Offset:  384 Size:    12 [unused]
//   float4 EyeMS_id36;                 // Offset:  400 Size:    16 [unused]
//   float4 PickingData_id76;           // Offset:  416 Size:    16
//
// }
//
//
// Resource Bindings:
//
// Name                                 Type  Format         Dim Slot Elements
// ------------------------------ ---------- ------- ----------- ---- --------
// PerDraw                           cbuffer      NA          NA    0        1
//
//
//
// Input signature:
//
// Name                 Index   Mask Register SysValue  Format   Used
// -------------------- ----- ------ -------- -------- ------- ------
// SV_Position              0   xyzw        0      POS   float       
//
//
// Output signature:
//
// Name                 Index   Mask Register SysValue  Format   Used
// -------------------- ----- ------ -------- -------- ------- ------
// SV_Target                0   xyzw        0   TARGET   float   xyzw
//
ps_5_0
dcl_globalFlags refactoringAllowed
dcl_constantbuffer cb0[27], immediateIndexed
dcl_output o0.xyzw
//
// Initial variable locations:
//   v0.x <- __input__.ShadingPosition_id0.x; v0.y <- __input__.ShadingPosition_id0.y; v0.z <- __input__.ShadingPosition_id0.z; v0.w <- __input__.ShadingPosition_id0.w; 
//   o0.x <- <PSMain return value>.ColorTarget_id1.x; o0.y <- <PSMain return value>.ColorTarget_id1.y; o0.z <- <PSMain return value>.ColorTarget_id1.z; o0.w <- <PSMain return value>.ColorTarget_id1.w
//
#line 173 "c:\aaa\projects\OpenEQ\OpenEQ\OpenEQ.Game\log\shader_XenkoEditorForwardShadingEffect_Picking_4b6bf459dee6d4fd80efba55449ac8ae.hlsl"
mov o0.xyzw, cb0[26].xyzw
ret 
// Approximately 2 instruction slots used
***************************
*************************/
const static int TMaxLightCount_id96 = 8;
const static int TMaxLightCount_id99 = 1;
const static int TCascadeCountBase_id162 = 4;
const static int TLightCountBase_id163 = 1;
const static int TCascadeCount_id167 = 4;
const static int TLightCount_id168 = 1;
const static int TOrder_id174 = 3;
struct PS_STREAMS 
{
    float4 ColorTarget_id1;
};
struct PS_OUTPUT 
{
    float4 ColorTarget_id1 : SV_Target0;
};
struct VS_STREAMS 
{
    float4 Position_id18;
    float3 meshNormal_id14;
    float4 PositionWS_id19;
    float4 PositionH_id21;
    float DepthVS_id20;
    float3 normalWS_id16;
    float4 ShadingPosition_id0;
};
struct VS_OUTPUT 
{
    float4 ShadingPosition_id0 : SV_Position;
};
struct VS_INPUT 
{
    float4 Position_id18 : POSITION;
    float3 meshNormal_id14 : NORMAL;
};
struct PointLightData 
{
    float3 PositionWS;
    float InvSquareRadius;
    float3 Color;
};
struct PointLightDataInternal 
{
    float3 PositionWS;
    float InvSquareRadius;
    float3 Color;
};
struct SpotLightData 
{
    float3 PositionWS;
    float3 DirectionWS;
    float3 AngleOffsetAndInvSquareRadius;
    float3 Color;
};
struct SpotLightDataInternal 
{
    float3 PositionWS;
    float3 DirectionWS;
    float3 AngleOffsetAndInvSquareRadius;
    float3 Color;
};
struct DirectionalLightData 
{
    float3 DirectionWS;
    float3 Color;
};
cbuffer PerDraw 
{
    float4x4 World_id29;
    float4x4 WorldInverse_id30;
    float4x4 WorldInverseTranspose_id31;
    float4x4 WorldView_id32;
    float4x4 WorldViewInverse_id33;
    float4x4 WorldViewProjection_id34;
    float3 WorldScale_id35;
    float4 EyeMS_id36;
    float4 PickingData_id76;
};
cbuffer PerMaterial 
{
    float2 scale_id185;
    float2 offset_id186;
};
cbuffer PerView 
{
    float4x4 View_id22;
    float4x4 ViewInverse_id23;
    float4x4 Projection_id24;
    float4x4 ProjectionInverse_id25;
    float4x4 ViewProjection_id26;
    float2 ProjScreenRay_id27;
    float4 Eye_id28;
    float NearClipPlane_id78 = 1.0f;
    float FarClipPlane_id79 = 100.0f;
    float2 ZProjection_id80;
    float2 ViewSize_id81;
    float AspectRatio_id82;
    float ClusterDepthScale_id88;
    float ClusterDepthBias_id89;
    int LightCount_id95;
    DirectionalLightData Lights_id97[TMaxLightCount_id96];
    int LightCount_id98;
    DirectionalLightData Lights_id100[TMaxLightCount_id99];
    float2 ShadowMapTextureSize_id102;
    float2 ShadowMapTextureTexelSize_id103;
    float4x4 WorldToShadowCascadeUV_id164[TCascadeCountBase_id162 * TLightCountBase_id163];
    float DepthBiases_id165[TLightCountBase_id163];
    float OffsetScales_id166[TLightCountBase_id163];
    float CascadeDepthSplits_id172[TCascadeCount_id167 * TLightCount_id168];
    float3 AmbientLight_id173;
    float3 SphericalColors_id175[TOrder_id174 * TOrder_id174];
    float MipCount_id176;
    float4x4 SkyMatrix_id178;
    float Intensity_id179;
};
cbuffer Globals 
{
    float2 Texture0TexelSize_id105;
    float2 Texture1TexelSize_id107;
    float2 Texture2TexelSize_id109;
    float2 Texture3TexelSize_id111;
    float2 Texture4TexelSize_id113;
    float2 Texture5TexelSize_id115;
    float2 Texture6TexelSize_id117;
    float2 Texture7TexelSize_id119;
    float2 Texture8TexelSize_id121;
    float2 Texture9TexelSize_id123;
};
void PostTransformPosition_id6()
{
}
void PreTransformPosition_id4()
{
}
void PostTransformPosition_id14(inout VS_STREAMS streams)
{
    PostTransformPosition_id6();
    streams.ShadingPosition_id0 = mul(streams.PositionWS_id19, ViewProjection_id26);
    streams.PositionH_id21 = streams.ShadingPosition_id0;
    streams.DepthVS_id20 = streams.ShadingPosition_id0.w;
}
void TransformPosition_id5()
{
}
void PreTransformPosition_id13(inout VS_STREAMS streams)
{
    PreTransformPosition_id4();
    streams.PositionWS_id19 = mul(streams.Position_id18, World_id29);
}
void BaseTransformVS_id7(inout VS_STREAMS streams)
{
    PreTransformPosition_id13(streams);
    TransformPosition_id5();
    PostTransformPosition_id14(streams);
}
void VSMain_id0()
{
}
void GenerateNormal_VS_id18(inout VS_STREAMS streams)
{
    streams.normalWS_id16 = mul(streams.meshNormal_id14, (float3x3)WorldInverseTranspose_id31);
}
void VSMain_id8(inout VS_STREAMS streams)
{
    VSMain_id0();
    BaseTransformVS_id7(streams);
}
PS_OUTPUT PSMain(VS_OUTPUT __input__)
{
    PS_STREAMS streams = (PS_STREAMS)0;
    streams.ColorTarget_id1 = PickingData_id76;
    PS_OUTPUT __output__ = (PS_OUTPUT)0;
    __output__.ColorTarget_id1 = streams.ColorTarget_id1;
    return __output__;
}
VS_OUTPUT VSMain(VS_INPUT __input__)
{
    VS_STREAMS streams = (VS_STREAMS)0;
    streams.Position_id18 = __input__.Position_id18;
    streams.meshNormal_id14 = __input__.meshNormal_id14;
    VSMain_id8(streams);
    GenerateNormal_VS_id18(streams);
    VS_OUTPUT __output__ = (VS_OUTPUT)0;
    __output__.ShadingPosition_id0 = streams.ShadingPosition_id0;
    return __output__;
}
