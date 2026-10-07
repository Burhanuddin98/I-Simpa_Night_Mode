import os, sys, stat

def size(path):
    total = 0
    try:
        with os.scandir(path) as it:
            for e in it:
                try:
                    st = e.stat(follow_symlinks=False)
                except OSError:
                    continue
                if st.st_file_attributes & stat.FILE_ATTRIBUTE_REPARSE_POINT:
                    continue
                if e.is_dir(follow_symlinks=False):
                    total += size(e.path)
                else:
                    total += st.st_size
    except OSError:
        pass
    return total

root = sys.argv[1]
depth = int(sys.argv[2]) if len(sys.argv) > 2 else 1
minGB = float(sys.argv[3]) if len(sys.argv) > 3 else 0.2

def walk(path, d):
    rows = []
    try:
        with os.scandir(path) as it:
            for e in it:
                try:
                    st = e.stat(follow_symlinks=False)
                except OSError:
                    continue
                if st.st_file_attributes & stat.FILE_ATTRIBUTE_REPARSE_POINT:
                    rows.append((0, e.path + "  [reparse]"))
                    continue
                if e.is_dir(follow_symlinks=False):
                    s = size(e.path)
                    rows.append((s, e.path))
                    if d > 1 and s / 1e9 >= minGB:
                        rows += walk(e.path, d - 1)
                else:
                    rows.append((st.st_size, e.path))
    except OSError:
        pass
    return rows

rows = walk(root, depth)
for s, p in sorted(rows, key=lambda r: -r[0]):
    if s / 1e9 >= minGB or p.endswith("[reparse]"):
        print(f"{s/1e9:8.2f} GB  {p}")
