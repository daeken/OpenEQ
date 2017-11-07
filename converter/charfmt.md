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
    - uint32 polygons
        - intvec3 indices
- uint32 vertexCount
- uint32 animations
    - string name
    - uint32 frames
        - vertexCount items
            - vec3 vertex
            - vec3 normal
            - vec2 texcoord
