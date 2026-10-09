const Pbf = require(process.argv[2] + '/pbf-3.2.1/package/dist/pbf.js');
const fs = require('fs');
const pbf = new Pbf(fs.readFileSync(process.argv[2] + '/all-the-cities-3.1.0/package/cities.pbf'));
let lastLat = 0, lastLon = 0; const out = [];
function readCity(tag, c, p) {
  if (tag === 2) c.name = p.readString(); else if (tag === 3) c.country = p.readString();
  else if (tag === 7) c.fc = p.readString(); else if (tag === 9) c.pop = p.readVarint();
  else if (tag === 10) { lastLon += p.readSVarint(); c.lon = lastLon / 1e5; }
  else if (tag === 11) { lastLat += p.readSVarint(); c.lat = lastLat / 1e5; }
  else if (tag === 1) p.readSVarint(); else if ([4,5,6,8].includes(tag)) p.readString();
}
while (pbf.pos < pbf.length) out.push(pbf.readMessage(readCity, { name: '', country: '', pop: 0 }));
const FR = new Set(['FR','GF','GP','MQ','RE','YT','NC','PF','DJ','AE','MC']);
const NB = new Set(['BE','LU','DE','CH','IT','ES','AD','GB','NL']);
const ZC = new Set(['GF','GP','MQ','RE','YT','NC','PF','DJ','AE']);
const sel = out.filter(c => c.fc !== 'PPLX' && (FR.has(c.country) && c.pop >= (ZC.has(c.country) ? 1000 : 1500)) || (NB.has(c.country) && c.pop >= 60000));
console.error('total', out.length, 'selected', sel.length, 'FR', sel.filter(c=>c.country==='FR').length);
sel.sort((a, b) => b.pop - a.pop);
fs.writeFileSync(process.argv[3], JSON.stringify(sel.map(c => [c.name, Math.round(c.lat*1000)/1000, Math.round(c.lon*1000)/1000, c.pop, FR.has(c.country) ? 1 : 0])));
