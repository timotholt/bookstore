#!/usr/bin/env python3
"""Strict, resumable local-demo import from the Open Library edition bulk dump.
Requires Python Pillow and psql. Never creates inventory or market-price claims.
"""
import argparse, gzip, hashlib, html, io, json, os, re, subprocess, time
from concurrent.futures import ThreadPoolExecutor, as_completed
from collections import Counter
from pathlib import Path
from urllib.parse import urlparse, unquote
from urllib.request import Request, urlopen
from PIL import Image

SOURCE = 'https://archive.org/download/ol_dump_2026-08-31/ol_dump_editions_2026-08-31.txt.gz'
def isbn13(value):
    value = re.sub(r'[-\s]', '', str(value)).upper()
    if len(value) == 10 and re.fullmatch(r'\d{9}[\dX]', value):
        if sum((10-i)*(10 if c == 'X' else int(c)) for i,c in enumerate(value)) % 11:
            return None
        value = '978' + value[:9]
        value += str((-sum(int(c)*(1 if i%2 == 0 else 3) for i,c in enumerate(value)))%10)
    if len(value) != 13 or not value.isdigit() or not value.startswith(('978','979')):
        return None
    return value if sum(int(c)*(1 if i%2 == 0 else 3) for i,c in enumerate(value))%10 == 0 else None

def clean(value):
    return re.sub(r'\s+', ' ', html.unescape(re.sub('<[^>]+>', ' ', str(value or '')))).strip()

def candidate(row, authors=None):
    isbn = next((n for x in row.get('isbn_13',[])+row.get('isbn_10',[]) if (n:=isbn13(x))),None)
    description=row.get('description','')
    if isinstance(description,dict): description=description.get('value','')
    title,desc=clean(row.get('title')),clean(description)
    author=re.sub(r'^by\s+', '', clean(row.get('by_statement')), flags=re.I).strip(' .') or 'Unknown author'
    years=re.findall(r'\b(?:1[4-9]\d{2}|20[0-2]\d)\b',str(row.get('publish_date','')))
    covers=[c for c in row.get('covers',[]) if isinstance(c,int) and c>0]
    fmt=clean(row.get('physical_format')) or 'Format unspecified'
    publishers=row.get('publishers',[])
    if not isbn or not title or not covers:
        return None
    if any(x in fmt.lower() for x in ('ebook','kindle','audio','cd')): return None
    price=14.99 if 'hard' in fmt.lower() else 9.99
    return dict(id='isbn-'+isbn,isbn=isbn,title=title,description=desc or 'Description unavailable.',author=author,authors=[author],
        year=int(years[-1]) if years else 0,format=fmt,publisher=clean(publishers[0]) if publishers else 'Unknown publisher',price=price,
        price_source='assigned_demo_usd_v1',source_id=row['key'],
        cover_source=f'https://covers.openlibrary.org/b/id/{covers[0]}-M.jpg?default=false',
        source='Open Library edition bulk dump',stock=0)

def verify_cover(row, cover_directory):
    try:
        cover_id=row['cover_source'].split('/')[-1].split('-')[0]
        body=(cover_directory/(cover_id+'.jpg')).read_bytes()
        if len(body)>5_000_000 or len(body)<1000:
            return None
        im=Image.open(io.BytesIO(body)); im.load()
        if im.width<70 or im.height<100:
            return None
        row['cover_sha256']=hashlib.sha256(body).hexdigest()
        row['cover_width'],row['cover_height']=im.size
        row['cover_url']=row['cover_source']
        row['cover_verified_at']=time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime())
        return row
    except Exception:
        return None

def verify_cover_metadata(row, metadata):
    cover_id=int(row['cover_source'].split('/')[-1].split('-')[0])
    dimensions=metadata.get(cover_id)
    if dimensions is None or dimensions[0]<70 or dimensions[1]<100: return None
    for attempt in range(1):
        try:
            request=Request(row['cover_source'],headers={'User-Agent':'ChantelsCornerCoverVerifier/1.0'})
            with urlopen(request,timeout=3) as response:
                body=response.read(5_000_001)
            if len(body)>5_000_000 or len(body)<1000:
                return None
            image=Image.open(io.BytesIO(body)); image.load()
            if image.width<70 or image.height<100:
                return None
            row.update(cover_url=row['cover_source'],cover_width=image.width,cover_height=image.height,
                cover_sha256=hashlib.sha256(body).hexdigest(),
                cover_verified_at=time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()),
                cover_proof='live_http_image_bytes_and_dimensions')
            return row
        except Exception:
            return None
    return None

def verify_metadata_row(item):
    row, metadata = item
    return verify_cover_metadata(row, metadata)

def prepare_metadata(args):
    rows=[json.loads(line) for line in args.candidates.read_text().splitlines()]
    excluded=set(args.exclude_isbns.read_text().splitlines()) if args.exclude_isbns else set()
    wanted={int(row['cover_source'].split('/')[-1].split('-')[0]) for row in rows}
    metadata={}
    with gzip.open(args.cover_metadata,'rt') as source:
        for line in source:
            fields=line.strip().split('\t')
            try:
                cover_id,width,height=map(int,fields[:3])
            except ValueError: continue
            if cover_id in wanted: metadata[cover_id]=(width,height)
    args.output.mkdir(parents=True,exist_ok=True)
    accepted_path=args.output/'accepted.jsonl'
    progress_path=args.output/'progress.json'
    accepted=[json.loads(line) for line in accepted_path.read_text().splitlines()] if accepted_path.exists() else []
    accepted_isbns={row['isbn'] for row in accepted}
    rejected=Counter()
    processed_offset=0
    if progress_path.exists():
        saved_progress=json.loads(progress_path.read_text())
        rejected.update(saved_progress.get('rejected_counts',{}))
        processed_offset=int(saved_progress.get('processed',0))
    eligible=[]
    seen=set(accepted_isbns)
    for row in rows:
        if not isbn13(row['isbn']) or row['isbn'] in seen or row['isbn'] in excluded: continue
        seen.add(row['isbn']); eligible.append((row, metadata))
    total_eligible=len(eligible)
    eligible=eligible[processed_offset:]
    processed=processed_offset
    batch_size=100
    with accepted_path.open('a') as output:
        for start in range(0,len(eligible),batch_size):
            batch=eligible[start:start+batch_size]
            with ThreadPoolExecutor(max_workers=128) as pool:
                futures=[pool.submit(verify_metadata_row, item) for item in batch]
                for future in as_completed(futures):
                    result=future.result()
                    if result:
                        accepted_isbns.add(result['isbn']); accepted.append(result)
                        output.write(json.dumps(result,ensure_ascii=False)+'\n'); output.flush()
                    else:
                        rejected['cover_live_verification_failed'] += 1
            processed += len(batch)
            progress=dict(target=args.count,processed=processed,accepted=len(accepted),
                rejected_counts=dict(rejected),remaining=total_eligible-processed,
                percent=round(processed/total_eligible*100,2) if total_eligible else 100)
            progress_path.write_text(json.dumps(progress,indent=2)+'\n')
            print(json.dumps(progress),flush=True)
            if len(accepted)>=args.count: break
    (args.output/'report.json').write_text(json.dumps(dict(target=args.count,accepted=len(accepted),complete=len(accepted)==args.count,
        candidates=len(rows),proof='ISBN checksums and required bibliographic metadata; every accepted cover was fetched and decoded from its live URL.',
        rejected=dict(rejected),
        price_policy='Assigned demo USD prices; no market-price claims',stock=0,source=SOURCE),indent=2)+'\n')
    if len(accepted)!=args.count: raise SystemExit('Target not met; nothing imported.')

def prepare(args):
    if args.candidates and args.cover_metadata:
        return prepare_metadata(args)
    if not args.editions or not args.covers:
        raise SystemExit('Supply --editions bulk dump and --covers directory from bulk cover archive; API crawling is not supported.')
    args.output.mkdir(parents=True,exist_ok=True)
    accepted_path=args.output/'accepted.jsonl'
    accepted=[json.loads(l) for l in accepted_path.read_text().splitlines()] if accepted_path.exists() else []
    seen={r['isbn'] for r in accepted}; hashes=Counter(r['cover_sha256'] for r in accepted)
    counts=Counter()
    with accepted_path.open('a') as out, gzip.open(args.editions,'rt') as rows:
        for raw in rows:
            if len(accepted)>=args.count: break
            counts['examined']+=1
            row=candidate(json.loads(raw.split('\t',4)[-1]))
            if row is None:
                counts['metadata_rejected']+=1; continue
            if row['isbn'] in seen: continue
            seen.add(row['isbn'])
            result=verify_cover(row,args.covers)
            if result is None: counts['cover_rejected']+=1; continue
            if hashes[result['cover_sha256']] >= 2:
                counts['duplicate_image_rejected']+=1; continue
            hashes[result['cover_sha256']]+=1
            out.write(json.dumps(result,ensure_ascii=False)+'\n'); out.flush(); accepted.append(result)
            if len(accepted)%100==0: print(json.dumps(dict(accepted=len(accepted),**counts)),flush=True)
    report=dict(target=args.count,accepted=len(accepted),complete=len(accepted)==args.count,counts=dict(counts),
                price_policy='Assigned demo USD prices: hardcover 14.99, other physical formats 9.99; not market offers.',
                inventory_policy='Stock is zero. No sellable inventory is fabricated.',source=SOURCE,
                public_reuse='Review source content and cover reuse terms before public deployment.')
    (args.output/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    if len(accepted)!=args.count: raise SystemExit('Target not met; nothing imported.')

def apply(args):
    rows=[json.loads(line) for line in (args.output/'accepted.jsonl').read_text().splitlines()]
    if len(rows)!=args.count or len({r['isbn'] for r in rows})!=args.count:
        raise SystemExit('Expected exact distinct target count; import refused.')
    if any(not all(r.get(k) for k in ('title','author','description','publisher','format','cover_url','source_id')) or not isbn13(r['isbn']) or not 0 <= int(r['year']) <= 2026 or not r.get('cover_verified_at') or r['stock']!=0 or not 0<r['price']<100 for r in rows):
        raise SystemExit('Invalid record; import refused.')
    url=os.environ.get('DATABASE_URL','')
    if urlparse(url).hostname not in ('localhost','127.0.0.1','::1') and urlparse(url).hostname != args.allow_database_host:
        raise SystemExit('Local databases only. Public deployment requires a separate reviewed operation.')
    payload=json.dumps(rows,ensure_ascii=False)
    marker='$import_'+hashlib.sha256(payload.encode()).hexdigest()+'$'
    sql="BEGIN; CREATE TEMP TABLE incoming AS SELECT * FROM jsonb_to_recordset("+marker+payload+marker+"::jsonb) AS x(id text,isbn text,title text,description text,author text,publisher text,year int,format text,price numeric,price_source text,source_id text,cover_url text,source text);\n"
    sql += """
DO $$ BEGIN
IF EXISTS (SELECT 1 FROM incoming i JOIN books b ON b.isbn=i.isbn WHERE b.id<>i.id) THEN
RAISE EXCEPTION 'Existing catalog ISBN collision; regenerate manifest excluding existing ISBNs';
END IF;
END $$;
INSERT INTO genres(slug,name) VALUES ('imported-books','Books') ON CONFLICT DO NOTHING;
INSERT INTO authors(slug,name,sort_name) SELECT DISTINCT 'import-'||md5(author),author,author FROM incoming ON CONFLICT(slug) DO NOTHING;
INSERT INTO books(id,slug,title,isbn,description,publisher,year,primary_author_id,primary_genre_id,cover_url,metadata_source,metadata_source_id,search_text,aspect_ratio)
SELECT i.id,i.id,i.title,i.isbn,i.description,i.publisher,i.year,a.id,g.id,i.cover_url,i.source,i.source_id,lower(i.title||' '||i.author||' '||i.isbn),0.667 FROM incoming i JOIN authors a ON a.slug='import-'||md5(i.author) CROSS JOIN genres g WHERE g.slug='imported-books'
ON CONFLICT(isbn) DO NOTHING;
INSERT INTO book_authors(book_id,author_id,role,position)
SELECT b.id,b.primary_author_id,'Author',1 FROM books b JOIN incoming i ON b.id=i.id ON CONFLICT DO NOTHING;
INSERT INTO book_genres(book_id,genre_id,is_primary)
SELECT b.id,b.primary_genre_id,true FROM books b JOIN incoming i ON b.id=i.id ON CONFLICT DO NOTHING;
INSERT INTO book_copies(book_id,condition,price,format,stock,notes,price_source)
SELECT b.id,'Good',i.price,i.format,0,'Demo catalog price only. No physical inventory has been verified.',i.price_source FROM incoming i JOIN books b ON b.isbn=i.isbn AND b.id=i.id ON CONFLICT(book_id,condition,format,price) DO NOTHING;
DO $$ BEGIN
IF (SELECT count(*) FROM books b JOIN incoming i ON b.id=i.id) <> (SELECT count(*) FROM incoming) THEN
RAISE EXCEPTION 'Imported book count does not match manifest';
END IF;
END $$;
SELECT count(*) AS imported_books FROM books b JOIN incoming i ON b.id=i.id;
COMMIT;
"""
    parsed=urlparse(url)
    env=dict(os.environ,PGHOST=parsed.hostname,PGPORT=str(parsed.port or 5432),PGDATABASE=parsed.path.lstrip('/'))
    if parsed.username: env['PGUSER']=unquote(parsed.username)
    if parsed.password: env['PGPASSWORD']=unquote(parsed.password)
    if parsed.hostname not in ('localhost','127.0.0.1','::1'): env['PGSSLMODE']='require'
    result=subprocess.run(['psql','-X','-v','ON_ERROR_STOP=1'],input=sql,text=True,env=env,capture_output=True)
    print(result.stdout)
    if result.returncode:
        error=result.stderr.replace(url,'[database URL redacted]')
        if parsed.password: error=error.replace(unquote(parsed.password),'[password redacted]')
        raise SystemExit(error)


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__); p.add_argument('command',choices=['prepare','apply']); p.add_argument('--exclude-isbns',type=Path); p.add_argument('--allow-database-host'); p.add_argument('--candidates',type=Path); p.add_argument('--cover-metadata',type=Path); p.add_argument('--editions',type=Path); p.add_argument('--covers',type=Path); p.add_argument('--count',type=int,default=10000); p.add_argument('--output',type=Path,default=Path('setup/artifacts/book-import'))
    a=p.parse_args(); (prepare if a.command=='prepare' else apply)(a)
