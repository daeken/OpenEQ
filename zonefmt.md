Types
=====

- string -- 7-bit encoded length followed by utf-8 bytes
- vecX -- vector of floats
- intvecX -- vector of integers

Structure
=========

- uint32 material_count
    - uint32 flags
    - string texturename
- uint32 object_count
    - string name -- Empty for zone
    - uint32 meshes
        - uint32 material_id
        - uint32 buffer_count
            - uint32 vertices
                - vec3 vertex
                - vec3 normal
                - vec2 texcoord
            - uint32 polygons [arrays in sequence.  one array of bools, one array of intvec3s]
                - bool collidable
                - intvec3 indices
