from itertools import chain, izip

def interleave(*lists):
    return list(chain.from_iterable(izip(*lists)))

def flatten(x):
    while len(x) and isinstance(x[0], list):
        x = [item for sub in x for item in sub]
    return x
