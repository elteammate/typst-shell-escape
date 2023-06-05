#import "shell-escape.typ": *

#let url-to-latex(latex-source) = {
  "https://latex.codecogs.com/svg.image?"
  encode-url(latex-source)
}

#http-get(
  url-to-latex(
```latex
\begin{align*}
\frac{\partial^2}{\partial t_1^2} f(t_0,t_1) = 
( \delta+2t_0+2t_1)^{\alpha( w-t_0+t_1 )-1} \cdot \bigl(  
\frac{\partial^2}{\partial t_1^2}\alpha(w-t_0+t_1) \cdot ( \delta+2t_0+2t_1) \cdot  \log ( \delta+2t_0+2t_1) +\\
\alpha'(w-t_0+t_1) \cdot 2 \cdot  \log ( \delta+2t_0+2t_1)+
\alpha'(w-t_0+t_1) \cdot ( \delta+2t_0+2t_1) \cdot  \frac{2}{\delta+2t_0+2t_1}\bigl)
\end{align*}
```.text
  ),
  method: image,
  format: ".svg",
)
