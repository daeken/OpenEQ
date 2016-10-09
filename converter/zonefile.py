import hashlib

class VertexBuffer(object):
    def __init__(self, data, count):
        self.data = data
        self.count = count
        assert len(data) % count == 0
        self.stride = len(data) / count
    
    def __getitem__(self, index):
        return self.data[index * self.stride:(index + 1) * self.stride]
    
    def __len__(self):
        return self.count

class Material(object):
    def __init__(self, flags, data):
        self.flags = flags
        self.filename = hashlib.sha256(data).hexdigest() + '.dds' 
        self.data = data

class Mesh(object):
    def __init__(self, material, vertbuffer, polygons):
        self.material = material
        self.vertbuffer = vertbuffer
        self.polygons = polygons
    
    def subset(self, cpoly):
        vbuffer = [[]]
        npoly = []
        used = {}
        
        def mapIndex(i):
            if i in used:
                return used[i]
            ind = used[i] = len(used)
            vbuffer[0] += self.vertbuffer[i]
            return ind
        
        for collidable, (a, b, c) in cpoly:
            npoly.append((collidable, (mapIndex(a), mapIndex(b), mapIndex(c))))
        
        return Mesh(self.material, VertexBuffer(vbuffer[0], len(used)), npoly)
    
    def optimize(self):
        def pushmesh():
            outmeshes.append(self.subset(cpoly))
        outmeshes = []
        cpoly, cverts = [], set()
        for collidable, (a, b, c) in self.polygons:
            na, nb, nc = a in cverts, b in cverts, c in cverts
            if len(cverts) + na + nb + nc > 65536:
                pushmesh()
                cpoly, cverts = [], set()
                na = nb = nc = True
            if na: cverts.add(a)
            if nb: cverts.add(b)
            if nc: cverts.add(c)
            cpoly.append((collidable, (a, b, c)))
        if len(cpoly):
            pushmesh()
        return outmeshes

class Object(object):
    def __init__(self, name=''):
        self.name = name
        self.meshes = []
    
    def addMesh(self, mesh):
        self.meshes.append(mesh)
        return self

class Zone(object):
    def __init__(self):
        self.objects = [Object()]
        self.zoneobj = self.objects[0]
    
    def addObject(self, name):
        obj = Object(name)
        self.objects.append(obj)
        return obj
    
    def output(self, zip):
        assets = {}
        for obj in self.objects:
            for mesh in obj.meshes:
                material = mesh.material
                if material.filename not in assets:
                    assets[material.filename] = material.data
        for k, v in assets.items():
            zip.writestr(k, v)
        
        for obj in self.objects:
            obj.meshes = [x for m in obj.meshes for x in m.optimize()]
