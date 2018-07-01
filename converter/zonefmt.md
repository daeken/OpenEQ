Types
=====

- string -- 7-bit encoded length followed by utf-8 bytes
- vecX -- vector of floats
- intvecX -- vector of 32-bit integers

Structure
=========

- uint32 material_count
    - uint32 flags
    - uint32 texture_count
        - string texturename
- uint32 object_count -- First is always the zone itself
    - uint32 meshes
        - uint32 material_id -- index into material array
        - bool32 collidable
        - uint32 vertices -- Separate arrays
            - vec3 vertex
            - vec3 normal
            - vec2 texcoord
        - uint32 polygons
            - intvec3 indices
- uint placeable_count
    - uint32 id -- index into object array
    - vec3 position
    - vec3 rotation -- radians around axis
    - vec3 scale
- uint light_count
    - vec3 position
    - vec3 color
    - float32 radius
    - float32 attenuation
    - uint flags
