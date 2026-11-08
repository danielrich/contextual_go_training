"""Read-only SGF corpus import and clearly identified introductory positions."""
import hashlib
from pathlib import Path
import gzip
import tarfile
import zipfile
from go_core import parse_sgf, eligible, replay


def import_text(store,text,source):
    g=parse_sgf(text)
    if not eligible(g): raise ValueError('Verified live pool requires explicit komi, a known live time control, supported rules (Japanese, Chinese or AGA), and no handicap.')
    if len(g['moves'])<12: raise ValueError('Game needs at least 12 moves.')
    replay(g,len(g['moves']))
    g.update(id=hashlib.sha256(text.encode()).hexdigest()[:20],source=source,sample=False)
    store.add_game(g)
    return g['id']


def import_path(store,path,limit=5000):
    path=Path(path).expanduser(); counts=dict(imported=0,duplicates=0,skipped=0,errors=[],reasons={})
    seen=store.game_ids()
    def add(raw,name):
        if counts['imported']>=limit: return
        try:
            text=raw.decode('utf-8-sig',errors='replace')
            ident=hashlib.sha256(text.encode()).hexdigest()[:20]
            if ident in seen:
                counts['duplicates']+=1
                return
            import_text(store,text,name); counts['imported']+=1; seen.add(ident)
        except ValueError as e:
            counts['skipped']+=1
            counts['reasons'][str(e)]=counts['reasons'].get(str(e),0)+1
            if len(counts['errors'])<8: counts['errors'].append(f'{name}: {e}')
    if path.is_dir():
        for f in sorted(path.rglob('*.sgf')):
            if counts['imported']>=limit: break
            if f.stat().st_size<=5_000_000: add(f.read_bytes(),str(f))
    elif zipfile.is_zipfile(path):
        with zipfile.ZipFile(path) as archive:
            for info in archive.infolist():
                if counts['imported']>=limit: break
                if info.filename.lower().endswith('.sgf') and info.file_size<=5_000_000:
                    add(archive.read(info),f'{path.name}/{info.filename}')
    elif tarfile.is_tarfile(path):
        with tarfile.open(path,'r:*') as archive:
            for info in archive:
                if counts['imported']>=limit: break
                if info.isfile() and info.name.lower().endswith('.sgf') and info.size<=5_000_000:
                    add(archive.extractfile(info).read(),f'{path.name}/{info.name}')
    elif path.suffix=='.gz':
        with gzip.open(path,'rb') as f: add(f.read(5_000_001),str(path))
    else: add(path.read_bytes(),str(path))
    return counts


def samples():
    sequences={
      9:'C7 G3 C3 G7 E5 E7 D7 D8 C8 E8 F7 F8 G6 H6 G5 H5 F5 E4 D4 D3 C4 B4 B5 B3 C5 F3 E3 F2 E2 D2',
      13:'D10 K4 D4 K10 F10 H10 F4 H4 C7 K7 D8 D6 F7 H7 G9 G5 E9 E8 F9 G8 H9 J9 J10 J8 K9 L9 L10 L8',
      19:'D16 Q4 D4 Q16 C14 R6 F3 C6 C3 D3 E3 C4 C5 B5 B4 B3 D6 E5 F5 E6 F6 E7 G4 G3 H3 H4 J3 J4 K3 K4 O17 R14 F17 C10 Q10 O3 L16 K17 M17 M16 N16 N17 O16 P17 O15 P16'
    }
    result=[]
    from go_core import coord
    for size,sequence in sequences.items():
        nodes=[]
        for i,m in enumerate(sequence.split()):
            x,y=coord(m,size); nodes.append(';'+('B' if i%2==0 else 'W')+'['+chr(97+x)+chr(97+y)+']')
        g=parse_sgf(f'(;SZ[{size}]KM[6.5]RU[Chinese]'+''.join(nodes)+')')
        replay(g,len(g['moves']))
        g.update(id=f'intro-{size}',source='Constructed introductory position · not an OGS game',sample=True)
        result.append(g)
    return result
