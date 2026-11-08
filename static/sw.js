// build-release.sh replaces this token so each deployment has an atomic shell cache.
const CACHE='whole-board-shell-__BUILD__';
const ASSETS=['/','/index.html','/app.js','/grader.js','/offline.js','/preview.js','/style.css','/manifest.webmanifest','/icon-192.png','/icon-512.png','/icon-maskable-512.png'];
self.addEventListener('install',event=>event.waitUntil((async()=>{const cache=await caches.open(CACHE);try{await cache.addAll(ASSETS.map(url=>new Request(url,{cache:'reload'})));}catch(error){await caches.delete(CACHE);throw error;}})()));
self.addEventListener('activate',event=>event.waitUntil((async()=>{await Promise.all((await caches.keys()).filter(key=>key.startsWith('whole-board-shell-')&&key!==CACHE).map(key=>caches.delete(key)));await self.clients.claim();})()));
self.addEventListener('message',event=>{if(event.data?.type==='SKIP_WAITING')self.skipWaiting();});
self.addEventListener('fetch',event=>{
 const url=new URL(event.request.url);if(event.request.method!=='GET'||url.origin!==self.location.origin)return;
 if(url.pathname.startsWith('/api/')||url.pathname==='/health')return;
 if(event.request.mode==='navigate'){event.respondWith((async()=>{const shell=await(await caches.open(CACHE)).match('/');return shell||fetch(event.request);})());return;}
 if(ASSETS.includes(url.pathname))event.respondWith((async()=>{const cache=await caches.open(CACHE);return(await cache.match(url.pathname))||fetch(event.request);})());
});
