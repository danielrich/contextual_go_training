"""SGF mainline ingestion and small, explicit Go rules implementation."""
import re

LETTERS = 'ABCDEFGHJKLMNOPQRST'


def coord(move, size):
    if not isinstance(move,str) or not re.fullmatch(r'[A-HJ-T][1-9][0-9]?',move):
        raise ValueError('Use a board coordinate such as D4.')
    x,y=LETTERS.index(move[0]),size-int(move[1:])
    if not (0<=x<size and 0<=y<size): raise ValueError('Move is outside the board.')
    return x,y


def vertex(x,y,size): return LETTERS[x]+str(size-y)


def neighbors(x,y,n):
    return [(a,b) for a,b in [(x-1,y),(x+1,y),(x,y-1),(x,y+1)] if 0<=a<n and 0<=b<n]


def group_at(board,x,y):
    color=board[y][x]
    if not color: return set(),set()
    stones,libs=set(),set()
    stack=[(x,y)]
    while stack:
        p=stack.pop()
        if p in stones: continue
        stones.add(p)
        for a,b in neighbors(*p,len(board)):
            if board[b][a]==color and (a,b) not in stones: stack.append((a,b))
            elif not board[b][a]: libs.add((a,b))
    return stones,libs


def play(board,color,move,previous=None):
    result=[row[:] for row in board]
    if move=='pass': return result
    x,y=coord(move,len(board))
    if result[y][x]: raise ValueError('That intersection is occupied.')
    c=1 if color=='B' else 2
    result[y][x]=c
    for a,b in neighbors(x,y,len(board)):
        if result[b][a]==3-c:
            stones,libs=group_at(result,a,b)
            if not libs:
                for s,t in stones: result[t][s]=0
    if not group_at(result,x,y)[1]: raise ValueError('Suicide is not legal in this trainer.')
    if previous is not None and result==previous: raise ValueError('Immediate ko recapture is not legal.')
    return result


def parse_sgf(text):
    if not isinstance(text,str) or len(text)>5_000_000: raise ValueError('SGF must be text under 5 MB.')
    i=0
    def space():
        nonlocal i
        while i<len(text) and text[i].isspace(): i+=1
    def tree():
        nonlocal i
        space()
        if i>=len(text) or text[i]!='(': raise ValueError('Expected an SGF game tree.')
        i+=1; nodes=[]; children=[]
        while True:
            space()
            if i>=len(text): raise ValueError('Unclosed SGF game tree.')
            if text[i]==';':
                i+=1; node={}
                while True:
                    space()
                    if i>=len(text) or not text[i].isupper(): break
                    start=i
                    while i<len(text) and text[i].isupper(): i+=1
                    key=text[start:i]; values=[]; space()
                    while i<len(text) and text[i]=='[':
                        i+=1; chars=[]
                        while i<len(text) and text[i]!=']':
                            if text[i]=='\\':
                                i+=1
                                if i<len(text) and text[i] in '\r\n':
                                    if text[i]=='\r' and i+1<len(text) and text[i+1]=='\n': i+=1
                                    i+=1; continue
                            if i<len(text): chars.append(text[i]); i+=1
                        if i>=len(text): raise ValueError('Unclosed SGF property.')
                        i+=1; values.append(''.join(chars)); space()
                    if not values: raise ValueError('SGF property lacks a value.')
                    node[key]=values
                nodes.append(node)
            elif text[i]=='(': children.append(tree())
            elif text[i]==')':
                i+=1
                if not nodes: raise ValueError('Empty SGF tree.')
                return nodes+(children[0] if children else [])
            else: raise ValueError('Malformed SGF.')
    try: nodes=tree()
    except RecursionError: raise ValueError('SGF variations are too deeply nested.')
    space()
    if i!=len(text): raise ValueError('Import one SGF game per file.')
    root=nodes[0]
    def prop(key,default=''): return root.get(key,[default])[0]
    try:
        size=int(prop('SZ','19')); komi=float(prop('KM','6.5')); handicap=int(prop('HA','0'))
    except ValueError: raise ValueError('Invalid board size, komi or handicap.')
    if size not in [9,13,19]: raise ValueError('Only 9×9, 13×13 and 19×19 are supported.')
    if not (-100<=komi<=100): raise ValueError('Invalid komi.')
    def convert(p):
        if p=='' or (p=='tt' and size<=19): return 'pass'
        if not re.fullmatch('[a-s]{2}',p): raise ValueError('Invalid SGF move.')
        x,y=ord(p[0])-97,ord(p[1])-97
        if x>=size or y>=size: raise ValueError('SGF move is outside board.')
        return vertex(x,y,size)
    setup=[]
    for color,key in [('B','AB'),('W','AW')]:
        for p in root.get(key,[]):
            if ':' in p:
                a,b=p.split(':'); ax,ay=coord(convert(a),size); bx,by=coord(convert(b),size)
                setup.extend([color,vertex(x,y,size)] for x in range(ax,bx+1) for y in range(ay,by+1))
            else: setup.append([color,convert(p)])
    moves=[]
    for j,node in enumerate(nodes):
        if j and any(k in node for k in ['AB','AW','AE','PL']): raise ValueError('Mid-game setup edits are not supported.')
        if 'B' in node and 'W' in node: raise ValueError('A node cannot contain both players’ moves.')
        for color in ['B','W']:
            if color in node: moves.append([color,convert(node[color][0])])
    try: tm=float(prop('TM','0'))
    except ValueError: tm=0
    ot=prop('OT').lower()
    # Fail closed for daily controls, Canadian/day controls and unknown controls.
    daily=any(s in ot for s in ['day','hour','86400','correspondence'])
    overtime_numbers=[float(n) for n in re.findall(r'\d+(?:\.\d+)?',ot)]
    daily=daily or any(n>=3600 for n in overtime_numbers)
    speed='live' if 0<tm<=7200 and not daily else ('correspondence' if tm>7200 or daily else 'unknown')
    archive_speed=prop('GC').lower().split(',')[0].strip()
    if archive_speed in ['live','correspondence','blitz','rapid']:
        speed=archive_speed if not daily else 'correspondence'
    source_url=prop('SO')
    ogs_url=re.search(r'https://online-go\.com/game/\d+',prop('PC'))
    if ogs_url: source_url=ogs_url[0]
    rules_raw=prop('RU','').lower()
    rules='chinese' if 'chinese' in rules_raw else 'aga' if 'aga' in rules_raw else None if 'tromp' in rules_raw else 'japanese' if 'japan' in rules_raw else None
    return dict(size=size,komi=komi,komi_known='KM' in root,rules=rules,rules_label=prop('RU','Japanese'),moves=moves,setup=setup,
                initial_player=prop('PL','W' if handicap else 'B'),handicap=handicap,speed=speed,
                black=prop('PB','Black'),white=prop('PW','White'),black_rank=prop('BR','Unknown'),
                white_rank=prop('WR','Unknown'),date=prop('DT'),source_url=source_url,time_control=prop('TM')+' '+prop('OT'))


def eligible(game):
    return game['speed']=='live' and game['handicap']==0 and game['rules'] is not None and game.get('komi_known',False)


def replay(game,turn):
    board=[[0]*game['size'] for _ in range(game['size'])]
    for c,m in game['setup']:
        if m=='pass': raise ValueError('Setup stones cannot pass.')
        x,y=coord(m,game['size'])
        if board[y][x]: raise ValueError('Overlapping setup stones.')
        board[y][x]=1 if c=='B' else 2
    history=[board]
    expected=game['initial_player']
    for c,m in game['moves'][:turn]:
        if c!=expected: raise ValueError('Non-alternating move sequence.')
        board=play(board,c,m,history[-2] if len(history)>1 else None)
        if m!='pass' and any(board==old for old in history): raise ValueError('Repeated board position is not supported.')
        history.append(board)
        expected='W' if c=='B' else 'B'
    return board


def to_play(game,turn):
    return ('W' if game['moves'][turn-1][0]=='B' else 'B') if turn else game['initial_player']


def rank_number(rank):
    m=re.fullmatch(r'\s*(\d+)\s*([kd])(?:yu|an)?\s*',rank.lower())
    return (-int(m[1]) if m[2]=='k' else int(m[1])-1) if m else None
