import test from 'node:test';
import assert from 'node:assert/strict';
import {grade,validCompleteProblem,GRADER_VERSION} from '../static/grader.js';
import {problem} from './fixtures.js';
test('unevaluated legal moves stay pending and never count as wrong',()=>{const f=grade(problem(),'candidates',['D4','E5','pass']);assert.equal(f.pending,true);assert.equal(f.correct,null);assert.deepEqual(f.missing,['pass']);});
test('White candidate quality uses White perspective and includes pass',()=>{const p=problem();p.analysis.candidates.pass={move:'pass',score:-1,visits:200,pv:[]};const f=grade(p,'candidates',['D4','E5','pass']);assert.equal(f.pending,false);assert.equal(f.correct,false);assert.equal(f.candidates[2].loss,3);});
test('complete offline readiness requires every legal move',()=>{const p=problem();p.analysis.complete=true;assert.equal(validCompleteProblem(p),false);p.analysis.candidates.pass={move:'pass',score:-4,visits:200,pv:[]};assert.equal(validCompleteProblem(p),true);});
test('counting includes signed score and its three-point tolerance',()=>{assert.equal(grade(problem(),'score',-1).correct,true);assert.equal(grade(problem(),'score',0).correct,false);});
test('duplicate and illegal candidate selections are rejected',()=>{assert.throws(()=>grade(problem(),'candidates',['D4','D4','pass']));assert.throws(()=>grade(problem(),'candidates',['A9','E5','pass']));});
