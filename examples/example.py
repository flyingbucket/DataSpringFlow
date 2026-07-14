import shutil
import pydsf as dsf

svc = dsf.DSFService()

# admin delete some global data
id = "imagenet@indexed"
refed = svc.check_is_referenced(id)
meta = svc.query_meta(id)
assert len(meta) == 1
meta = meta[0]
path = meta.metadata.path
if len(refed) == 0:
    # in future version we shuld implemnt these mark_* methods
    # to ensure tractioness between disk data and metadata
    # svc.mark_deleting(id)
    shutil.rmtree(path)  # this may take a few minuits on old hdd
    # during deleting, we should prevent other users referencing this dataset
    # or create downstream datasets based on it
    svc.delete_metadata(id)
