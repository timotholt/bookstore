#!/usr/bin/env python3
"""Stream a pinned official Open Library bulk snapshot; stop at candidate target.
No Open Library API crawling. Final cover verification is a separate import step.
"""
import argparse
import gzip
import json
import time
from pathlib import Path
from urllib.request import Request, urlopen
from import_books import SOURCE, candidate

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--count',type=int,default=13000)
    parser.add_argument('--output',type=Path,default=Path('setup/artifacts/book-import/candidates.jsonl'))
    args=parser.parse_args()
    args.output.parent.mkdir(parents=True,exist_ok=True)
    examined=0; seen=set(); started=time.time()
    request=Request(SOURCE,headers={'User-Agent':'ChantelsCornerBulkImport/1.0'})
    with urlopen(request,timeout=120) as response,gzip.open(response,'rt') as rows,args.output.open('w') as output:
        for line in rows:
            examined+=1
            row=candidate(json.loads(line.split('\t',4)[-1]))
            if row and row['isbn'] not in seen:
                seen.add(row['isbn']); output.write(json.dumps(row)+'\n')
            if examined%100000==0:
                output.flush(); print(json.dumps(dict(examined=examined,candidates=len(seen),seconds=int(time.time()-started))),flush=True)
            if len(seen)>=args.count:break
    print(json.dumps(dict(examined=examined,candidates=len(seen),source=SOURCE)),flush=True)
    if len(seen)<args.count:raise SystemExit('Candidate target not met; no database changes made.')

if __name__=='__main__':main()
