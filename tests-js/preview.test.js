import test from 'node:test';
import assert from 'node:assert/strict';
import {replayVariation} from '../static/preview.js';
test('variation preview alternates colors, captures stones, numbers moves, and leaves the problem untouched',()=>{const p={size:3,to_play:'B',board:[[0,1,0],[1,2,0],[0,1,0]]};const before=structuredClone(p);const v=replayVariation(p,['C2','pass','B2'],3);assert.deepEqual(p,before);assert.equal(v.board[1][1],1);assert.equal(v.board[1][2],1);assert.deepEqual(v.markers.map(m=>m.number),[1,3]);assert.equal(v.passes[0].number,2);assert.equal(v.played,3);assert.equal(v.error,null);});
test('variation steps show only the selected prefix and stop safely at invalid continuations',()=>{const p={size:3,to_play:'W',board:[[0,0,0],[0,0,0],[0,0,0]]};assert.equal(replayVariation(p,['A3','B3'],1).board[0][1],0);const v=replayVariation(p,['A3','A3'],2);assert.equal(v.played,1);assert.match(v.error,/occupied/i);assert.equal(v.board[0][0],2);});
