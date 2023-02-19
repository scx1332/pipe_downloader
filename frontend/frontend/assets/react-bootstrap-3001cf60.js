import{c as g}from"./classnames-5c20d0be.js";import{r as n,j as p}from"./react-5c397423.js";const b=["xxl","xl","lg","md","sm","xs"],B="xs",C=n.createContext({prefixes:{},breakpoints:b,minBreakpoint:B});function R(s,e){const{prefixes:r}=n.useContext(C);return s||r[e]||e}function j(s,e){let r=0;return n.Children.map(s,a=>n.isValidElement(a)?e(a,r++):a)}const P=1e3,y={min:0,max:100,animated:!1,isChild:!1,visuallyHidden:!1,striped:!1};function I(s,e,r){const a=(s-e)/(r-e)*100;return Math.round(a*P)/P}function N({min:s,now:e,max:r,label:a,visuallyHidden:l,striped:d,animated:i,className:c,style:m,variant:o,bsPrefix:t,...u},x){return p.jsx("div",{ref:x,...u,role:"progressbar",className:g(c,`${t}-bar`,{[`bg-${o}`]:o,[`${t}-bar-animated`]:i,[`${t}-bar-striped`]:i||d}),style:{width:`${I(e,s,r)}%`,...m},"aria-valuenow":e,"aria-valuemin":s,"aria-valuemax":r,children:l?p.jsx("span",{className:"visually-hidden",children:a}):a})}const h=n.forwardRef(({isChild:s,...e},r)=>{if(e.bsPrefix=R(e.bsPrefix,"progress"),s)return N(e,r);const{min:a,now:l,max:d,label:i,visuallyHidden:c,striped:m,animated:o,bsPrefix:t,variant:u,className:x,children:f,...E}=e;return p.jsx("div",{ref:r,...E,className:g(x,t),children:f?j(f,v=>n.cloneElement(v,{isChild:!0})):N({min:a,now:l,max:d,label:i,visuallyHidden:c,striped:m,animated:o,bsPrefix:t,variant:u},r)})});h.displayName="ProgressBar";h.defaultProps=y;export{h as P};
