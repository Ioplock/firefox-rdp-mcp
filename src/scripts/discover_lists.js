JSON.stringify((()=>{
          const path=el=>{const parts=[];for(let d=0;el&&el.nodeType===1&&d<7;el=el.parentElement,d++){
            if(el.id){parts.unshift('#'+CSS.escape(el.id));break;}
            const tag=el.localName;let n=1;for(let s=el.previousElementSibling;s;s=s.previousElementSibling)if(s.localName===tag)n++;
            parts.unshift(tag+':nth-of-type('+n+')');
          }return parts.join(' > ');};
          const found=[];
          for(const parent of document.querySelectorAll('main, section, div, ul, ol, tbody, [role=list], [role=grid]')){
            const groups=new Map();
            for(const el of parent.children){if(!el.querySelector('a,h2,h3,[role=link],td'))continue;
              const key=el.localName;const group=groups.get(key)||[];group.push(el);groups.set(key,group);}
            for(const [tag,rows] of groups){if(rows.length<3)continue;
              const first=rows[0],fields={text:{select:'',max:300}};
              const title=['h2','h3','[itemprop=name]','a[title]','a'].find(s=>first.querySelector(s));
              if(title) fields.title=title;
              if(first.querySelector('a[href]')) fields.link={select:'a[href]',attr:'href'};
              const price=['[itemprop=price]','[data-price]','[class*=price]'].find(s=>first.querySelector(s));
              if(price) fields.price_candidate=price;
              const item=path(parent)+' > '+tag;
              found.push({item,count:rows.length,extract_example:{item,fields,limit:5},price_note:price?'price_candidate is a heuristic; verify units and discounts':undefined,samples:rows.slice(0,2).map(e=>({text:(e.innerText||'').replace(/\s+/g,' ').slice(0,160),has_link:!!e.querySelector('a'),has_heading:!!e.querySelector('h2,h3')}))});}
          }
          return found.sort((a,b)=>b.count-a.count).slice(0,8);
        })())
