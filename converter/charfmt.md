Types
=====

- string -- 7-bit encoded length followed by utf-8 bytes
- vecX -- vector of floats
- intvecX -- vector of 32-bit integers

Structure
=========

- uint32 mesh_count
    - uint32 material_flags
    - uint32 texture_count
        - string texturename
    - uint32 vertices -- Separate arrays
        - vec3 vertex
        - vec3 normal
        - vec2 texcoord
        - uint32 bonenum
    - uint32 polygons
        - intvec3 indices
- uint32 bone_count
    - int32 parent # -1 indicates root
- uint32 animations
    - string name
    - bone_count items
        - uint32 frames
            - vec3 position
            - vec4 rotation
