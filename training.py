"""Transparent grading; scores are always Black's lead including komi."""
from go_core import replay, play, to_play


def candidate_loss(baseline,candidate,player):
    return max(0,(baseline-candidate)*(1 if player=='B' else -1))


def winner_grade(answer,score):
    return answer==('even' if abs(score)<=2 else 'B' if score>0 else 'W')


def score_grade(answer,score): return abs(answer-score)<=3


def group_estimate(ownership,color):
    values=[v*(1 if color=='B' else -1) for v in ownership]
    if all(v>=.9 for v in values): return 'secure-looking'
    if all(v<=-.7 for v in values): return 'at-risk'
    return 'uncertain'


def validate_candidates(game,turn,moves):
    if not isinstance(moves,list) or len(moves)!=3 or any(not isinstance(m,str) for m in moves) or len(set(moves))!=3:
        raise ValueError('Choose three different legal moves.')
    board=replay(game,turn)
    previous=replay(game,turn-1) if turn else None
    history=[replay(game,i) for i in range(turn+1)]
    for move in moves:
        if move=='pass': continue
        candidate=play(board,to_play(game,turn),move,previous)
        if any(candidate==old for old in history): raise ValueError('This move repeats an earlier position.')
