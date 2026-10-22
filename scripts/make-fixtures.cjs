// Deterministic, original test PDFs; no WPS data or proprietary assets.
const fs = require('node:fs');
const crypto = require('node:crypto');
const path = require('node:path');
const out = path.join(__dirname, '..', 'tests', 'fixtures');
fs.mkdirSync(out, { recursive: true });
const padding = Buffer.from('28bf4e5e4e758a4164004e56fffa01082e2e00b6d0683e802f0ca9fe6453697a', 'hex');
const md5 = (...parts) => crypto.createHash('md5').update(Buffer.concat(parts)).digest();
function pad(password) { return Buffer.concat([Buffer.from(password, 'latin1'), padding]).subarray(0, 32); }
function rc4(key, data) {
  const s = Array.from({length:256}, (_, i) => i); let j=0;
  for (let i=0;i<256;i++) { j=(j+s[i]+key[i%key.length])&255; [s[i],s[j]]=[s[j],s[i]]; }
  let i=0; j=0; const result=Buffer.alloc(data.length);
  for (let n=0;n<data.length;n++) { i=(i+1)&255; j=(j+s[i])&255;[s[i],s[j]]=[s[j],s[i]];result[n]=data[n]^s[(s[i]+s[j])&255]; }
  return result;
}
function document(encrypted=false, nested=false, many=false, linked=false) {
  const id = md5(Buffer.from('fpdf original reading fixture v1'));
  const owner = rc4(md5(pad('owner')).subarray(0,5), pad('readme'));
  const permission = Buffer.alloc(4); permission.writeInt32LE(-4);
  const key = md5(pad('readme'), owner, permission, id).subarray(0,5);
  const user = rc4(key, padding);
  const stream = (object, text) => {
    let bytes=Buffer.from(text,'ascii');
    if(encrypted) { const suffix=Buffer.alloc(5);suffix.writeUIntLE(object,0,3);bytes=rc4(md5(key,suffix).subarray(0,10),bytes); }
    return Buffer.concat([Buffer.from(`<< /Length ${bytes.length} >>\nstream\n`),bytes,Buffer.from('\nendstream')]);
  };
  const text = (x,y,size,str) => `BT /F1 ${size} Tf ${x} ${y} Td (${str}) Tj ET\n`;
  const page1 = [
    '1 1 1 rg 0 0 612 792 re f',
    '0.85 0.20 0.21 rg 0 775 612 17 re f',
    '0.85 0.20 0.21 rg', text(46,719,18,'fpdf / READING NOTES'),
    '0.16 0.19 0.24 rg',text(46,662,34,'A clearer way to read.'),
    '0.45 0.49 0.55 rg',text(46,626,12,'Open a document. Find your place. Focus on what matters.'),
    '0.93 0.94 0.96 rg 46 460 520 125 re f',
    '0.16 0.19 0.24 rg',text(66,550,16,'01   Start with the essentials'),
    text(66,518,12,'Page navigation, zoom, rotation and searchable page text.'),
    text(66,494,12,'A Rust desktop reader with a dedicated PDF render worker.'),
    '0.85 0.20 0.21 rg',text(46,411,18,'Read at your own pace'),
    '0.16 0.19 0.24 rg',text(46,374,12,'Use Page Down to continue to the landscape chart.'),
    text(46,350,12,'Use Ctrl+F to find the word essentials on this page.'),
    text(46,326,12,'Zoom in to inspect vector text and lines.'),
    '0.87 0.88 0.91 RG 46 105 m 566 105 l S',
    '0.45 0.49 0.55 rg',text(46,74,10,'FPDF SAMPLE DOCUMENT'),text(521,74,10,'1 / 3'),
  ].join('\n');
  const page2 = ['1 1 1 rg 0 0 792 612 re f','0.85 0.20 0.21 rg 0 595 792 17 re f',
    '0.16 0.19 0.24 rg',text(46,535,28,'A different perspective.'),text(46,498,12,'Landscape pages keep their own aspect ratio.'),
    '0.85 0.20 0.21 rg 100 150 110 140 re f','0.91 0.42 0.40 rg 270 150 110 220 re f',
    '0.96 0.67 0.61 rg 440 150 110 280 re f','0.98 0.82 0.78 rg 610 150 90 180 re f',
    '0.16 0.19 0.24 rg',text(120,119,12,'Open'),text(290,119,12,'Navigate'),text(460,119,12,'Explore'),text(620,119,12,'Focus'),
    '0.45 0.49 0.55 rg',text(46,55,10,'FPDF SAMPLE DOCUMENT'),text(700,55,10,'2 / 3')].join('\n');
  const page3 = ['1 1 1 rg 0 0 612 792 re f','0.85 0.20 0.21 rg 0 775 612 17 re f','0.16 0.19 0.24 rg',
    text(46,711,29,'Every page has a story.'),text(46,665,13,'This is the third and final page.'),
    text(46,626,12,'Try rotating this page, then return to fit width.'),
    text(46,597,12,'The text panel lets you select and copy extracted text.'),
    '0.93 0.94 0.96 rg 46 260 520 270 re f','0.85 0.20 0.21 rg',text(72,474,21,'Small tools. Clear purpose.'),
    '0.16 0.19 0.24 rg',text(72,431,12,'The application and sample document are independent'),text(72,407,12,'implementations. No WPS runtime DLL is required.'),
    '0.45 0.49 0.55 rg',text(46,74,10,'FPDF SAMPLE DOCUMENT'),text(521,74,10,'3 / 3')].join('\n');
  const objs = [null,
    '<< /Type /Catalog /Pages 2 0 R /Outlines 10 0 R >>',
    '<< /Type /Pages /Kids [3 0 R 5 0 R 7 0 R] /Count 3 >>',
    '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 9 0 R >> >> /Contents 4 0 R >>',
    stream(4,page1),
    '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 792 612] /Resources << /Font << /F1 9 0 R >> >> /Contents 6 0 R >>',
    stream(6,page2),
    '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 9 0 R >> >> /Contents 8 0 R >>',
    stream(8,page3),
    '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',
    '<< /Type /Outlines /First 11 0 R /Last 13 0 R /Count 3 >>',
    '<< /Title (01 Essentials) /Parent 10 0 R /Next 12 0 R /Dest [3 0 R /Fit] >>',
    '<< /Title (02 Landscape) /Parent 10 0 R /Prev 11 0 R /Next 13 0 R /Dest [5 0 R /Fit] >>',
    '<< /Title (03 Final page) /Parent 10 0 R /Prev 12 0 R /Dest [7 0 R /Fit] >>',
  ];
  if(linked) {
    objs[3]=objs[3].replace('/Contents 4 0 R', '/Annots [14 0 R 15 0 R 16 0 R 17 0 R] /Contents 4 0 R');
    objs.push('<< /Type /Annot /Subtype /Link /Rect [46 400 420 432] /Border [0 0 0] /Dest [7 0 R /XYZ 46 600 null] >>');
    objs.push('<< /Type /Annot /Subtype /Link /Rect [46 310 420 340] /Border [0 0 0] /A << /S /GoTo /D (chapter-two) >> >>');
    objs.push('<< /Type /Annot /Subtype /Link /Rect [46 340 420 365] /Border [0 0 0] /A << /S /URI /URI (https://example.invalid) >> >>');
    objs.push('<< /Type /Annot /Subtype /Link /Rect [46 370 420 395] /Dest [99 /Fit] >>');
    objs[1]=objs[1].replace('/Outlines 10 0 R','/Names << /Dests << /Names [(chapter-two) [5 0 R /Fit]] >> >> /Outlines 10 0 R');
  }
  if(many) {
    objs[5] = '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 9 0 R >> >> /Contents 4 0 R >>';
    const refs=['3 0 R','5 0 R','7 0 R'];
    for(let page=3;page<16;page++) {
      refs.push(`${objs.length} 0 R`);
      objs.push('<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 9 0 R >> >> /Contents 4 0 R >>');
    }
    objs[2]=`<< /Type /Pages /Kids [${refs.join(' ')}] /Count 16 >>`;
  }
  if(nested) {
    objs[10] = '<< /Type /Outlines /First 11 0 R /Last 13 0 R /Count 5 >>';
    objs[11] = '<< /Title (01 Essentials) /Parent 10 0 R /Next 12 0 R /Dest [3 0 R /Fit] /First 14 0 R /Last 14 0 R /Count 2 >>';
    objs.push('<< /Title (01.1 Reading tools) /Parent 11 0 R /Dest [5 0 R /Fit] /First 15 0 R /Last 15 0 R /Count 1 >>');
    objs.push('<< /Title (01.1.1 Final page) /Parent 14 0 R /Dest [7 0 R /Fit] >>');
  }
  if(encrypted) {
    // Outline title strings must be encrypted too; using hex strings avoids
    // PDF literal-string escaping and keeps the sample deterministic.
    for(let object=11;object<=13;object++) {
      const suffix=Buffer.alloc(5);suffix.writeUIntLE(object,0,3);
      objs[object]=objs[object].replace(/\/Title \(([^)]+)\)/,(_,title)=>
        `/Title <${rc4(md5(key,suffix).subarray(0,10),Buffer.from(title)).toString('hex')}>`);
    }
    objs.push(`<< /Filter /Standard /V 1 /R 2 /O <${owner.toString('hex')}> /U <${user.toString('hex')}> /P -4 >>`);
  }
  const chunks=[Buffer.from('%PDF-1.4\n')],offsets=[0];let length=chunks[0].length;
  for(let i=1;i<objs.length;i++){offsets.push(length);const b=Buffer.concat([Buffer.from(`${i} 0 obj\n`),Buffer.from(objs[i]),Buffer.from('\nendobj\n')]);chunks.push(b);length+=b.length;}
  const xref=length;
  chunks.push(Buffer.from(`xref\n0 ${objs.length}\n0000000000 65535 f \n`+offsets.slice(1).map(o=>`${String(o).padStart(10,'0')} 00000 n \n`).join('')+
    `trailer\n<< /Size ${objs.length} /Root 1 0 R /ID [<${id.toString('hex')}> <${id.toString('hex')}>] ${encrypted?'/Encrypt 14 0 R ':''}>>\nstartxref\n${xref}\n%%EOF\n`));
  return Buffer.concat(chunks);
}
fs.writeFileSync(path.join(out,'reading-demo.pdf'),document());
fs.writeFileSync(path.join(out,'linked-toc.pdf'),document(false,false,false,true));
fs.writeFileSync(path.join(out,'nested-outline.pdf'),document(false,true));
fs.writeFileSync(path.join(out,'grid-demo.pdf'),document(false,false,true));
fs.writeFileSync(path.join(out,'password.pdf'),document(true));
fs.writeFileSync(path.join(out,'invalid.pdf'),'This is deliberately not a PDF.\n');
console.log('Created original 3-page fixtures; password.pdf user password: readme');
