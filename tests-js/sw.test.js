import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
test('an active worker serves its own HTML shell while an update waits',async()=>{const listeners={};const self={location:{origin:'https://practice.example'},addEventListener:(name,fn)=>listeners[name]=fn};const caches={open:async()=>({match:async key=>key==='/'?new Response('old shell'):undefined})};vm.runInNewContext(fs.readFileSync(new URL('../static/sw.js',import.meta.url),'utf8'),{self,caches,URL,Request,Response,fetch:async()=>new Response('new incompatible shell')});let response;listeners.fetch({request:{method:'GET',url:'https://practice.example/',mode:'navigate'},respondWith:value=>response=value});assert.equal(await(await response).text(),'old shell');});
