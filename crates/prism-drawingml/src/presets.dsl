# Preset shape geometry (ECMA-376 Part 1, presetShapeDefinitions), written in
# the compact DSL parsed by geom.rs. Angles are in 60000ths of a degree.

@rect
path
M l t; L r t; L r b; L l b; Z

@roundRect
av adj 16667
gd a pin 0 adj 50000
gd x1 */ ss a 100000
gd x2 +- r 0 x1
gd y2 +- b 0 x1
gd il */ x1 29289 100000
gd ir +- r 0 il
gd ib +- b 0 il
tx il il ir ib
path
M l x1; A x1 x1 cd2 cd4; L x2 t; A x1 x1 3cd4 cd4; L r y2; A x1 x1 0 cd4; L x1 b; A x1 x1 cd4 cd4; Z

@snip1Rect
av adj 16667
gd a pin 0 adj 50000
gd dx2 */ ss a 100000
gd x1 +- r 0 dx2
path
M l t; L x1 t; L r dx2; L r b; L l b; Z

@snip2SameRect
av adj1 16667
av adj2 0
gd a1 pin 0 adj1 50000
gd a2 pin 0 adj2 50000
gd tx1 */ ss a1 100000
gd tx2 +- r 0 tx1
gd bx1 */ ss a2 100000
gd bx2 +- r 0 bx1
gd by1 +- b 0 bx1
path
M tx1 t; L tx2 t; L r tx1; L r by1; L bx2 b; L bx1 b; L l by1; L l tx1; Z

@round1Rect
av adj 16667
gd a pin 0 adj 50000
gd dx1 */ ss a 100000
gd x1 +- r 0 dx1
path
M l t; L x1 t; A dx1 dx1 3cd4 cd4; L r b; L l b; Z

@round2SameRect
av adj1 16667
av adj2 0
gd a1 pin 0 adj1 50000
gd a2 pin 0 adj2 50000
gd tx1 */ ss a1 100000
gd tx2 +- r 0 tx1
gd bx1 */ ss a2 100000
gd bx2 +- r 0 bx1
gd by1 +- b 0 bx1
path
M tx1 t; L tx2 t; A tx1 tx1 3cd4 cd4; L r by1; A bx1 bx1 0 cd4; L bx1 b; A bx1 bx1 cd4 cd4; L l tx1; A tx1 tx1 cd2 cd4; Z

@round2DiagRect
av adj1 16667
av adj2 0
gd a1 pin 0 adj1 50000
gd a2 pin 0 adj2 50000
gd x1 */ ss a1 100000
gd y1 +- b 0 x1
gd a */ ss a2 100000
gd x2 +- r 0 a
gd y2 +- b 0 a
path
M x1 t; L x2 t; A a a 3cd4 cd4; L r y1; A x1 x1 0 cd4; L a b; A a a cd4 cd4; L l x1; A x1 x1 cd2 cd4; Z

@ellipse
gd idx cos wd2 2700000
gd idy sin hd2 2700000
gd il +- hc 0 idx
gd ir +- hc idx 0
gd it +- vc 0 idy
gd ib +- vc idy 0
tx il it ir ib
path
M l vc; A wd2 hd2 cd2 cd4; A wd2 hd2 3cd4 cd4; A wd2 hd2 0 cd4; A wd2 hd2 cd4 cd4; Z

@triangle
av adj 50000
gd a pin 0 adj 100000
gd x1 */ w a 200000
gd x2 */ w a 100000
gd x3 +- x1 wd2 0
tx x1 vc x3 b
path
M l b; L x2 t; L r b; Z

@rtTriangle
gd it */ h 7 12
gd ir */ w 7 12
gd ib */ h 11 12
tx wd12 it ir ib
path
M l b; L l t; L r b; Z

@diamond
gd ir */ w 3 4
gd ib */ h 3 4
tx wd4 hd4 ir ib
path
M l vc; L hc t; L r vc; L hc b; Z

@parallelogram
av adj 25000
gd maxAdj */ 100000 w ss
gd a pin 0 adj maxAdj
gd x1 */ ss a 200000
gd x2 */ ss a 100000
gd x6 +- r 0 x2
gd x5 +- r 0 x1
path
M l b; L x2 t; L r t; L x6 b; Z

@trapezoid
av adj 25000
gd maxAdj */ 50000 w ss
gd a pin 0 adj maxAdj
gd x2 */ ss a 100000
gd x3 +- r 0 x2
path
M l b; L x2 t; L x3 t; L r b; Z

@pentagon
av hf 105146
av vf 110557
gd swd2 */ wd2 hf 100000
gd shd2 */ hd2 vf 100000
gd svc */ vc vf 100000
gd dx1 cos swd2 1080000
gd dx2 cos swd2 18360000
gd dy1 sin shd2 1080000
gd dy2 sin shd2 18360000
gd x1 +- hc 0 dx1
gd x2 +- hc 0 dx2
gd x3 +- hc dx2 0
gd x4 +- hc dx1 0
gd y1 +- svc 0 dy1
gd y2 +- svc 0 dy2
path
M x1 y1; L hc t; L x4 y1; L x3 y2; L x2 y2; Z

@hexagon
av adj 25000
av vf 115470
gd maxAdj */ 50000 w ss
gd a pin 0 adj maxAdj
gd shd2 */ hd2 vf 100000
gd x1 */ ss a 100000
gd x2 +- r 0 x1
gd dy1 sin shd2 3600000
gd y1 +- vc 0 dy1
gd y2 +- vc dy1 0
path
M l vc; L x1 y1; L x2 y1; L r vc; L x2 y2; L x1 y2; Z

@octagon
av adj 29289
gd a pin 0 adj 50000
gd x1 */ ss a 100000
gd x2 +- r 0 x1
gd y2 +- b 0 x1
path
M l x1; L x1 t; L x2 t; L r x1; L r y2; L x2 b; L x1 b; L l y2; Z

@plus
av adj 25000
gd a pin 0 adj 50000
gd x1 */ ss a 100000
gd x2 +- r 0 x1
gd y2 +- b 0 x1
path
M l x1; L x1 x1; L x1 t; L x2 t; L x2 x1; L r x1; L r y2; L x2 y2; L x2 b; L x1 b; L x1 y2; L l y2; Z

@star4
av adj 12500
gd a pin 0 adj 50000
gd iwd2 */ wd2 a 50000
gd ihd2 */ hd2 a 50000
gd sdx cos iwd2 2700000
gd sdy sin ihd2 2700000
gd sx1 +- hc 0 sdx
gd sx2 +- hc sdx 0
gd sy1 +- vc 0 sdy
gd sy2 +- vc sdy 0
path
M l vc; L sx1 sy1; L hc t; L sx2 sy1; L r vc; L sx2 sy2; L hc b; L sx1 sy2; Z

@star5
av adj 19098
av hf 105146
av vf 110557
gd a pin 0 adj 50000
gd swd2 */ wd2 hf 100000
gd shd2 */ hd2 vf 100000
gd svc */ vc vf 100000
gd dx1 cos swd2 1080000
gd dx2 cos swd2 18360000
gd dy1 sin shd2 1080000
gd dy2 sin shd2 18360000
gd x1 +- hc 0 dx1
gd x2 +- hc 0 dx2
gd x3 +- hc dx2 0
gd x4 +- hc dx1 0
gd y1 +- svc 0 dy1
gd y2 +- svc 0 dy2
gd iwd2 */ swd2 a 50000
gd ihd2 */ shd2 a 50000
gd sdx1 cos iwd2 20520000
gd sdx2 cos iwd2 3240000
gd sdy1 sin ihd2 3240000
gd sdy2 sin ihd2 20520000
gd sx1 +- hc 0 sdx1
gd sx2 +- hc 0 sdx2
gd sx3 +- hc sdx2 0
gd sx4 +- hc sdx1 0
gd sy1 +- svc 0 sdy1
gd sy2 +- svc 0 sdy2
gd sy3 +- svc ihd2 0
path
M x1 y1; L sx2 sy1; L hc t; L sx3 sy1; L x4 y1; L sx4 sy2; L x3 y2; L hc sy3; L x2 y2; L sx1 sy2; Z

@rightArrow
av adj1 50000
av adj2 50000
gd maxAdj2 */ 100000 w ss
gd a1 pin 0 adj1 100000
gd a2 pin 0 adj2 maxAdj2
gd dx1 */ ss a2 100000
gd x1 +- r 0 dx1
gd dy1 */ h a1 200000
gd y1 +- vc 0 dy1
gd y2 +- vc dy1 0
path
M l y1; L x1 y1; L x1 t; L r vc; L x1 b; L x1 y2; L l y2; Z

@leftArrow
av adj1 50000
av adj2 50000
gd maxAdj2 */ 100000 w ss
gd a1 pin 0 adj1 100000
gd a2 pin 0 adj2 maxAdj2
gd dx2 */ ss a2 100000
gd x2 +- l dx2 0
gd dy1 */ h a1 200000
gd y1 +- vc 0 dy1
gd y2 +- vc dy1 0
path
M l vc; L x2 t; L x2 y1; L r y1; L r y2; L x2 y2; L x2 b; Z

@upArrow
av adj1 50000
av adj2 50000
gd maxAdj2 */ 100000 h ss
gd a1 pin 0 adj1 100000
gd a2 pin 0 adj2 maxAdj2
gd dy2 */ ss a2 100000
gd y2 +- t dy2 0
gd dx1 */ w a1 200000
gd x1 +- hc 0 dx1
gd x2 +- hc dx1 0
path
M l y2; L hc t; L r y2; L x2 y2; L x2 b; L x1 b; L x1 y2; Z

@downArrow
av adj1 50000
av adj2 50000
gd maxAdj2 */ 100000 h ss
gd a1 pin 0 adj1 100000
gd a2 pin 0 adj2 maxAdj2
gd dy1 */ ss a2 100000
gd y1 +- b 0 dy1
gd dx1 */ w a1 200000
gd x1 +- hc 0 dx1
gd x2 +- hc dx1 0
path
M l y1; L x1 y1; L x1 t; L x2 t; L x2 y1; L r y1; L hc b; Z

@leftRightArrow
av adj1 50000
av adj2 50000
gd maxAdj2 */ 50000 w ss
gd a1 pin 0 adj1 100000
gd a2 pin 0 adj2 maxAdj2
gd x2 */ ss a2 100000
gd x3 +- r 0 x2
gd dy */ h a1 200000
gd y1 +- vc 0 dy
gd y2 +- vc dy 0
path
M l vc; L x2 t; L x2 y1; L x3 y1; L x3 t; L r vc; L x3 b; L x3 y2; L x2 y2; L x2 b; Z

@upDownArrow
av adj1 50000
av adj2 50000
gd maxAdj2 */ 50000 h ss
gd a1 pin 0 adj1 100000
gd a2 pin 0 adj2 maxAdj2
gd y2 */ ss a2 100000
gd y3 +- b 0 y2
gd dx1 */ w a1 200000
gd x1 +- hc 0 dx1
gd x2 +- hc dx1 0
path
M l y2; L hc t; L r y2; L x2 y2; L x2 y3; L r y3; L hc b; L l y3; L x1 y3; L x1 y2; Z

@quadArrow
av adj1 22500
av adj2 22500
av adj3 22500
gd a2 pin 0 adj2 50000
gd maxAdj1 */ a2 2 1
gd a1 pin 0 adj1 maxAdj1
gd q1 +- 100000 0 maxAdj1
gd maxAdj3 */ q1 1 2
gd a3 pin 0 adj3 maxAdj3
gd x1 */ ss a3 100000
gd dx2 */ ss a2 100000
gd x2 +- hc 0 dx2
gd x5 +- hc dx2 0
gd dx3 */ ss a1 200000
gd x3 +- hc 0 dx3
gd x4 +- hc dx3 0
gd x6 +- r 0 x1
gd y2 +- vc 0 dx2
gd y5 +- vc dx2 0
gd y3 +- vc 0 dx3
gd y4 +- vc dx3 0
gd y6 +- b 0 x1
path
M l vc; L x1 y2; L x1 y3; L x3 y3; L x3 x1; L x2 x1; L hc t; L x5 x1; L x4 x1; L x4 y3; L x6 y3; L x6 y2; L r vc; L x6 y5; L x6 y4; L x4 y4; L x4 y6; L x5 y6; L hc b; L x2 y6; L x3 y6; L x3 y4; L x1 y4; L x1 y5; Z

@notchedRightArrow
av adj1 50000
av adj2 50000
gd maxAdj2 */ 100000 w ss
gd a1 pin 0 adj1 100000
gd a2 pin 0 adj2 maxAdj2
gd dx2 */ ss a2 100000
gd x2 +- r 0 dx2
gd dy1 */ h a1 200000
gd y1 +- vc 0 dy1
gd y2 +- vc dy1 0
gd x1 */ dy1 dx2 hd2
path
M l y1; L x2 y1; L x2 t; L r vc; L x2 b; L x2 y2; L l y2; L x1 vc; Z

@chevron
av adj 50000
gd maxAdj */ 100000 w ss
gd a pin 0 adj maxAdj
gd x1 */ ss a 100000
gd x2 +- r 0 x1
path
M l t; L x2 t; L r vc; L x2 b; L l b; L x1 vc; Z

@homePlate
av adj 50000
gd maxAdj */ 100000 w ss
gd a pin 0 adj maxAdj
gd dx1 */ ss a 100000
gd x1 +- r 0 dx1
path
M l t; L x1 t; L r vc; L x1 b; L l b; Z

@line
path fill=none
M l t; L r b

@bentConnector3
av adj1 50000
gd x1 */ w adj1 100000
path fill=none
M l t; L x1 t; L x1 b; L r b

@curvedConnector3
av adj1 50000
gd x2 */ w adj1 100000
gd x1 +/ l x2 2
gd x3 +/ r x2 2
gd y3 */ h 3 4
path fill=none
M l t; C x1 t x2 hd4 x2 vc; C x2 y3 x3 b r b

@donut
av adj 25000
gd a pin 0 adj 50000
gd dr */ ss a 100000
gd iwd2 +- wd2 0 dr
gd ihd2 +- hd2 0 dr
path
M l vc; A wd2 hd2 cd2 cd4; A wd2 hd2 3cd4 cd4; A wd2 hd2 0 cd4; A wd2 hd2 cd4 cd4; Z
M dr vc; A iwd2 ihd2 cd2 -5400000; A iwd2 ihd2 cd4 -5400000; A iwd2 ihd2 0 -5400000; A iwd2 ihd2 3cd4 -5400000; Z

@frame
av adj1 12500
gd a1 pin 0 adj1 50000
gd x1 */ ss a1 100000
gd x4 +- r 0 x1
gd y4 +- b 0 x1
path
M l t; L r t; L r b; L l b; Z
M x1 x1; L x1 y4; L x4 y4; L x4 x1; Z

@heart
gd dx1 */ w 49 48
gd dx2 */ w 10 48
gd x1 +- hc 0 dx1
gd x2 +- hc 0 dx2
gd x3 +- hc dx2 0
gd x4 +- hc dx1 0
gd y1 +- t 0 hd3
path
M hc hd4; C x3 y1 x4 hd4 hc b; C x1 hd4 x2 y1 hc hd4; Z

@can
av adj 25000
gd maxAdj */ 50000 h ss
gd a pin 0 adj maxAdj
gd y1 */ ss a 200000
gd y2 +- y1 y1 0
gd y3 +- b 0 y1
tx l y2 r y3
path stroke=false
M l y1; A wd2 y1 cd2 -10800000; L r y3; A wd2 y1 0 cd2; Z
path stroke=false fill=lighten
M l y1; A wd2 y1 cd2 cd2; A wd2 y1 0 cd2; Z
path fill=none
M r y1; A wd2 y1 0 cd2; A wd2 y1 cd2 cd2; L r y3; A wd2 y1 0 cd2; L l y1

@cube
av adj 25000
gd a pin 0 adj 100000
gd y1 */ ss a 100000
gd y4 +- b 0 y1
gd x4 +- r 0 y1
tx l y1 x4 b
path stroke=false
M l y1; L x4 y1; L x4 b; L l b; Z
path stroke=false fill=darkenLess
M x4 y1; L r t; L r y4; L x4 b; Z
path stroke=false fill=lightenLess
M l y1; L y1 t; L r t; L x4 y1; Z
path fill=none
M l y1; L y1 t; L r t; L r y4; L x4 b; L l b; Z; M l y1; L x4 y1; L r t; M x4 y1; L x4 b

@foldedCorner
av adj 16667
gd a pin 0 adj 50000
gd dy2 */ ss a 100000
gd dy1 */ dy2 1 5
gd x1 +- r 0 dy2
gd x2 +- x1 dy1 0
gd y2 +- b 0 dy2
gd y1 +- y2 dy1 0
path stroke=false
M l t; L r t; L r y2; L x1 b; L l b; Z
path stroke=false fill=darkenLess
M x1 b; L x2 y1; L r y2; Z
path fill=none
M x1 b; L x2 y1; L r y2; L x1 b; L l b; L l t; L r t; L r y2

@teardrop
av adj 100000
gd a pin 0 adj 200000
gd r2 sqrt 2
gd tw */ r2 wd2 1
gd th */ r2 hd2 1
gd sw */ tw a 100000
gd sh */ th a 100000
gd dx1 cos sw 2700000
gd dy1 sin sh 2700000
gd x1 +- hc dx1 0
gd y1 +- vc 0 dy1
gd x2 +/ hc x1 2
gd y2 +/ vc y1 2
path
M l vc; A wd2 hd2 cd2 cd4; Q x2 t x1 y1; Q r y2 r vc; A wd2 hd2 0 cd4; A wd2 hd2 cd4 cd4; Z

@pie
av adj1 0
av adj2 16200000
gd stAng pin 0 adj1 21599999
gd enAng pin 0 adj2 21599999
gd sw1 +- enAng 0 stAng
gd sw2 +- sw1 21600000 0
gd swAng ?: sw1 sw1 sw2
gd wt1 sin wd2 stAng
gd ht1 cos hd2 stAng
gd dx1 cat2 wd2 ht1 wt1
gd dy1 sat2 hd2 ht1 wt1
gd x1 +- hc dx1 0
gd y1 +- vc dy1 0
path
M x1 y1; A wd2 hd2 stAng swAng; L hc vc; Z

@chord
av adj1 2700000
av adj2 16200000
gd stAng pin 0 adj1 21599999
gd enAng pin 0 adj2 21599999
gd sw1 +- enAng 0 stAng
gd sw2 +- sw1 21600000 0
gd swAng ?: sw1 sw1 sw2
gd wt1 sin wd2 stAng
gd ht1 cos hd2 stAng
gd dx1 cat2 wd2 ht1 wt1
gd dy1 sat2 hd2 ht1 wt1
gd x1 +- hc dx1 0
gd y1 +- vc dy1 0
path
M x1 y1; A wd2 hd2 stAng swAng; Z

@arc
av adj1 16200000
av adj2 0
gd stAng pin 0 adj1 21599999
gd enAng pin 0 adj2 21599999
gd sw1 +- enAng 0 stAng
gd sw2 +- sw1 21600000 0
gd swAng ?: sw1 sw1 sw2
gd wt1 sin wd2 stAng
gd ht1 cos hd2 stAng
gd dx1 cat2 wd2 ht1 wt1
gd dy1 sat2 hd2 ht1 wt1
gd x1 +- hc dx1 0
gd y1 +- vc dy1 0
path fill=none
M x1 y1; A wd2 hd2 stAng swAng

@blockArc
av adj1 10800000
av adj2 0
av adj3 25000
gd stAng pin 0 adj1 21599999
gd istAng pin 0 adj2 21599999
gd a3 pin 0 adj3 50000
gd sw11 +- istAng 0 stAng
gd sw12 +- sw11 21600000 0
gd swAng ?: sw11 sw11 sw12
gd iswAng +- 0 0 swAng
gd wt1 sin wd2 stAng
gd ht1 cos hd2 stAng
gd dx1 cat2 wd2 ht1 wt1
gd dy1 sat2 hd2 ht1 wt1
gd x1 +- hc dx1 0
gd y1 +- vc dy1 0
gd dr */ ss a3 100000
gd iwd2 +- wd2 0 dr
gd ihd2 +- hd2 0 dr
gd wt3 sin iwd2 istAng
gd ht3 cos ihd2 istAng
gd dx3 cat2 iwd2 ht3 wt3
gd dy3 sat2 ihd2 ht3 wt3
gd x3 +- hc dx3 0
gd y3 +- vc dy3 0
path
M x1 y1; A wd2 hd2 stAng swAng; L x3 y3; A iwd2 ihd2 istAng iswAng; Z

@smileyFace
av adj 4653
gd a pin -4653 adj 4653
gd x1 */ w 4969 21699
gd x2 */ w 6215 21600
gd x3 */ w 13135 21600
gd x4 */ w 16640 21600
gd y1 */ h 7570 21600
gd y3 */ h 16515 21600
gd dy2 */ h a 100000
gd y2 +- y3 0 dy2
gd y4 +- y3 dy2 0
gd dy3 */ h a 50000
gd y5 +- y4 dy3 0
gd wR */ w 1125 21600
gd hR */ h 1125 21600
path
M l vc; A wd2 hd2 cd2 21600000; Z
path fill=darkenLess
M x2 y1; A wR hR cd2 21600000; Z; M x3 y1; A wR hR cd2 21600000; Z
path fill=none
M x1 y2; Q hc y5 x4 y2

@bracketPair
av adj 16667
gd a pin 0 adj 50000
gd x1 */ ss a 100000
gd x2 +- r 0 x1
gd y2 +- b 0 x1
path stroke=false
M l x1; A x1 x1 cd2 cd4; L x2 t; A x1 x1 3cd4 cd4; L r y2; A x1 x1 0 cd4; L x1 b; A x1 x1 cd4 cd4; Z
path fill=none
M x1 b; A x1 x1 cd4 cd4; L l x1; A x1 x1 cd2 cd4; M x2 t; A x1 x1 3cd4 cd4; L r y2; A x1 x1 0 cd4

@leftBracket
av adj 8333
gd maxAdj */ 50000 h ss
gd a pin 0 adj maxAdj
gd y1 */ ss a 100000
gd y2 +- b 0 y1
path fill=none
M r b; A w y1 cd4 cd4; L l y1; A w y1 cd2 cd4

@rightBracket
av adj 8333
gd maxAdj */ 50000 h ss
gd a pin 0 adj maxAdj
gd y1 */ ss a 100000
gd y2 +- b 0 y1
path fill=none
M l t; A w y1 3cd4 cd4; L r y2; A w y1 0 cd4

@leftBrace
av adj1 8333
av adj2 50000
gd a2 pin 0 adj2 100000
gd q1 +- 100000 0 a2
gd q2 min q1 a2
gd q3 */ q2 1 2
gd maxAdj1 */ q3 h ss
gd a1 pin 0 adj1 maxAdj1
gd y1 */ ss a1 100000
gd y3 */ h a2 100000
gd y2 +- y3 0 y1
gd y4 +- y3 y1 0
path fill=none
M r b; A wd2 y1 cd4 cd4; L hc y4; A wd2 y1 0 -5400000; A wd2 y1 cd4 -5400000; L hc y1; A wd2 y1 cd2 cd4

@rightBrace
av adj1 8333
av adj2 50000
gd a2 pin 0 adj2 100000
gd q1 +- 100000 0 a2
gd q2 min q1 a2
gd q3 */ q2 1 2
gd maxAdj1 */ q3 h ss
gd a1 pin 0 adj1 maxAdj1
gd y1 */ ss a1 100000
gd y3 */ h a2 100000
gd y2 +- y3 0 y1
gd y4 +- b 0 y1
path fill=none
M l t; A wd2 y1 3cd4 cd4; L hc y2; A wd2 y1 cd2 -5400000; A wd2 y1 3cd4 -5400000; L hc y4; A wd2 y1 0 cd4

@wave
av adj1 12500
gd a1 pin 0 adj1 20000
gd y1 */ h a1 100000
gd dy2 */ y1 10 3
gd y2 +- y1 0 dy2
gd y3 +- y1 dy2 0
gd y4 +- b 0 y1
gd y5 +- y4 0 dy2
gd y6 +- y4 dy2 0
gd x3 */ w 1 3
gd x4 */ w 2 3
tx l y3 r y5
path
M l y1; C x3 y2 x4 y3 r y1; L r y4; C x4 y6 x3 y5 l y4; Z

@lightningBolt
path w=21600 h=21600
M 8458 0; L 12722 4304; L 11050 5040; L 15500 9750; L 13700 10450; L 21600 21600; L 10200 12850; L 11950 12150; L 5300 8350; L 7100 7650; L 0 3800; Z

@moon
av adj 50000
gd a pin 0 adj 87500
gd g0 */ ss a 100000
gd g0w */ g0 w ss
gd g1 +- ss 0 g0
gd g2 */ g0 g0 g1
gd g3 */ ss ss g1
gd g4 */ g3 2 1
gd g5 +- g4 0 g2
gd g6 +- g5 0 g0
gd g6w */ g6 w ss
gd g7 */ g5 1 2
gd g8 +- g7 0 g0
gd dy1 */ g8 hd2 ss
path
M r b; A w hd2 cd4 cd2; A g8 hd2 3cd4 -10800000; Z

@sun
av adj 25000
gd a pin 12500 adj 46875
gd dx */ ss a 100000
gd ir +- wd2 0 dx
gd irh +- hd2 0 dx
gd r1 */ wd2 92 100
gd r1h */ hd2 92 100
path
M dx vc; A ir irh cd2 21600000; Z

@plaque
av adj 16667
gd a pin 0 adj 50000
gd x1 */ ss a 100000
gd x2 +- r 0 x1
gd y2 +- b 0 x1
path
M l x1; A x1 x1 cd4 -5400000; L x2 t; A x1 x1 cd2 -5400000; L r y2; A x1 x1 3cd4 -5400000; L x1 b; A x1 x1 0 -5400000; Z

@bevel
av adj 12500
gd a pin 0 adj 50000
gd x1 */ ss a 100000
gd x2 +- r 0 x1
gd y2 +- b 0 x1
tx x1 x1 x2 y2
path stroke=false
M x1 x1; L x2 x1; L x2 y2; L x1 y2; Z
path stroke=false fill=lightenLess
M l t; L r t; L x2 x1; L x1 x1; Z
path stroke=false fill=darkenLess
M l b; L x1 y2; L x2 y2; L r b; Z
path stroke=false fill=lighten
M l t; L x1 x1; L x1 y2; L l b; Z
path stroke=false fill=darken
M r t; L r b; L x2 y2; L x2 x1; Z
path fill=none
M l t; L r t; L r b; L l b; Z; M x1 x1; L x2 x1; L x2 y2; L x1 y2; Z; M l t; L x1 x1; M l b; L x1 y2; M r t; L x2 x1; M r b; L x2 y2

@wedgeRectCallout
av adj1 -20833
av adj2 62500
gd dxPos */ w adj1 100000
gd dyPos */ h adj2 100000
gd xPos +- hc dxPos 0
gd yPos +- vc dyPos 0
gd dq */ dxPos h w
gd ady abs dyPos
gd adq abs dq
gd dz +- ady 0 adq
gd xg1 ?: dxPos 7 2
gd xg2 ?: dxPos 10 5
gd x1 */ w xg1 12
gd x2 */ w xg2 12
gd yg1 ?: dyPos 7 2
gd yg2 ?: dyPos 10 5
gd y1 */ h yg1 12
gd y2 */ h yg2 12
gd t1 ?: dxPos l xPos
gd xl ?: dz l t1
gd t2 ?: dyPos x1 xPos
gd xt ?: dz t2 x1
gd t3 ?: dxPos xPos r
gd xr ?: dz r t3
gd t4 ?: dyPos xPos x1
gd xb ?: dz t4 x1
gd t5 ?: dxPos y1 yPos
gd yl ?: dz y1 t5
gd t6 ?: dyPos t yPos
gd yt ?: dz t6 t
gd t7 ?: dxPos yPos y1
gd yr ?: dz y1 t7
gd t8 ?: dyPos yPos b
gd yb ?: dz t8 b
path
M l t; L x1 t; L xt yt; L x2 t; L r t; L r y1; L xr yr; L r y2; L r b; L x2 b; L xb yb; L x1 b; L l b; L l y2; L xl yl; L l y1; Z

@wedgeRoundRectCallout
av adj1 -20833
av adj2 62500
av adj3 16667
gd dxPos */ w adj1 100000
gd dyPos */ h adj2 100000
gd xPos +- hc dxPos 0
gd yPos +- vc dyPos 0
gd dq */ dxPos h w
gd ady abs dyPos
gd adq abs dq
gd dz +- ady 0 adq
gd xg1 ?: dxPos 7 2
gd xg2 ?: dxPos 10 5
gd x1 */ w xg1 12
gd x2 */ w xg2 12
gd yg1 ?: dyPos 7 2
gd yg2 ?: dyPos 10 5
gd y1 */ h yg1 12
gd y2 */ h yg2 12
gd t1 ?: dxPos l xPos
gd xl ?: dz l t1
gd t2 ?: dyPos x1 xPos
gd xt ?: dz t2 x1
gd t3 ?: dxPos xPos r
gd xr ?: dz r t3
gd t4 ?: dyPos xPos x1
gd xb ?: dz t4 x1
gd t5 ?: dxPos y1 yPos
gd yl ?: dz y1 t5
gd t6 ?: dyPos t yPos
gd yt ?: dz t6 t
gd t7 ?: dxPos yPos y1
gd yr ?: dz y1 t7
gd t8 ?: dyPos yPos b
gd yb ?: dz t8 b
gd u1 */ ss adj3 100000
gd u2 +- r 0 u1
gd v2 +- b 0 u1
path
M l u1; A u1 u1 cd2 cd4; L x1 t; L xt yt; L x2 t; L u2 t; A u1 u1 3cd4 cd4; L r y1; L xr yr; L r y2; L r v2; A u1 u1 0 cd4; L x2 b; L xb yb; L x1 b; L u1 b; A u1 u1 cd4 cd4; L l y2; L xl yl; L l y1; Z

@wedgeEllipseCallout
av adj1 -20833
av adj2 62500
gd dxPos */ w adj1 100000
gd dyPos */ h adj2 100000
gd xPos +- hc dxPos 0
gd yPos +- vc dyPos 0
gd sdx */ dxPos h 1
gd sdy */ dyPos w 1
gd pang at2 sdx sdy
gd stAng +- pang 660000 0
gd enAng +- pang 0 660000
gd dx1 cos wd2 stAng
gd dy1 sin hd2 stAng
gd x1 +- hc dx1 0
gd y1 +- vc dy1 0
gd dx2 cos wd2 enAng
gd dy2 sin hd2 enAng
gd x2 +- hc dx2 0
gd y2 +- vc dy2 0
gd stAng1 at2 dx1 dy1
gd enAng1 at2 dx2 dy2
gd swAng1 +- enAng1 0 stAng1
gd swAng2 +- swAng1 21600000 0
gd swAng ?: swAng1 swAng1 swAng2
path
M xPos yPos; L x1 y1; A wd2 hd2 stAng1 swAng; L xPos yPos; Z

@cloud
path w=43200 h=43200
M 3900 14370; C 3900 6000 11000 2000 16000 5000; C 18000 1000 24000 0 27000 3000; C 30000 0 36500 1000 37500 6500; C 42500 7500 43200 13000 41000 16000; C 43200 20000 42500 27000 38000 29000; C 39000 35000 34000 40000 28000 38000; C 25000 43200 17500 43200 15000 39000; C 10000 42000 4000 39000 4500 33000; C 0 32000 -500 24000 3500 22000; C 0 19000 0 15000 3900 14370; Z

@mathPlus
av adj1 23520
gd a1 pin 0 adj1 73490
gd dx1 */ w 73490 200000
gd dy1 */ h 73490 200000
gd dx2 */ ss a1 200000
gd x1 +- hc 0 dx1
gd x2 +- hc 0 dx2
gd x3 +- hc dx2 0
gd x4 +- hc dx1 0
gd y1 +- vc 0 dy1
gd y2 +- vc 0 dx2
gd y3 +- vc dx2 0
gd y4 +- vc dy1 0
path
M x1 y2; L x2 y2; L x2 y1; L x3 y1; L x3 y2; L x4 y2; L x4 y3; L x3 y3; L x3 y4; L x2 y4; L x2 y3; L x1 y3; Z

@mathMinus
av adj1 23520
gd a1 pin 0 adj1 100000
gd dy1 */ h a1 200000
gd dx1 */ w 73490 200000
gd y1 +- vc 0 dy1
gd y2 +- vc dy1 0
gd x1 +- hc 0 dx1
gd x2 +- hc dx1 0
path
M x1 y1; L x2 y1; L x2 y2; L x1 y2; Z

@mathEqual
av adj1 23520
av adj2 11760
gd a1 pin 0 adj1 36745
gd a2 pin 0 adj2 36745
gd dy1 */ h a1 100000
gd dy2 */ h a2 200000
gd dx1 */ w 73490 200000
gd y2 +- vc 0 dy2
gd y3 +- vc dy2 0
gd y1 +- y2 0 dy1
gd y4 +- y3 dy1 0
gd x1 +- hc 0 dx1
gd x2 +- hc dx1 0
path
M x1 y1; L x2 y1; L x2 y2; L x1 y2; Z; M x1 y3; L x2 y3; L x2 y4; L x1 y4; Z

@flowChartTerminator
path w=21600 h=21600
M 3475 0; L 18125 0; A 3475 10800 3cd4 cd2; L 3475 21600; A 3475 10800 cd4 cd2; Z

@flowChartAlternateProcess
gd x2 +- r 0 ssd6
gd y2 +- b 0 ssd6
path
M l ssd6; A ssd6 ssd6 cd2 cd4; L x2 t; A ssd6 ssd6 3cd4 cd4; L r y2; A ssd6 ssd6 0 cd4; L ssd6 b; A ssd6 ssd6 cd4 cd4; Z

@flowChartDocument
path w=21600 h=21600
M 0 0; L 21600 0; L 21600 17322; C 10800 17322 10800 23922 0 20172; Z

@flowChartInputOutput
path w=5 h=5
M 0 5; L 1 0; L 5 0; L 4 5; Z

@flowChartPredefinedProcess
path w=1 h=1 stroke=false
M 0 0; L 1 0; L 1 1; L 0 1; Z
path w=8 h=8 fill=none
M 1 0; L 1 8; M 7 0; L 7 8
path w=1 h=1 fill=none
M 0 0; L 1 0; L 1 1; L 0 1; Z

@flowChartManualInput
path w=5 h=5
M 0 1; L 5 0; L 5 5; L 0 5; Z

@flowChartManualOperation
path w=5 h=5
M 0 0; L 5 0; L 4 5; L 1 5; Z

@flowChartPreparation
path w=10 h=10
M 0 5; L 2 0; L 8 0; L 10 5; L 8 10; L 2 10; Z

@flowChartOffpageConnector
path w=10 h=10
M 0 0; L 10 0; L 10 8; L 5 10; L 0 8; Z

@flowChartMerge
path w=2 h=2
M 0 0; L 2 0; L 1 2; Z

@flowChartPunchedCard
path w=5 h=5
M 0 1; L 1 0; L 5 0; L 5 5; L 0 5; Z

@flowChartDelay
path
M l t; L hc t; A wd2 hd2 3cd4 cd2; L l b; Z

@flowChartMagneticDisk
gd y1 */ h 1 6
gd y3 */ h 5 6
path stroke=false
M l y1; A wd2 y1 cd2 -10800000; L r y3; A wd2 y1 0 cd2; Z
path fill=none
M r y1; A wd2 y1 0 cd2; A wd2 y1 cd2 cd2; L r y3; A wd2 y1 0 cd2; L l y1

@ribbon2
av adj1 16667
av adj2 50000
gd a1 pin 0 adj1 33333
gd a2 pin 25000 adj2 75000
gd x10 +- r 0 wd8
gd dx2 */ w a2 200000
gd x2 +- hc 0 dx2
gd x9 +- hc dx2 0
gd y1 */ h a1 100000
gd y2 +- b 0 y1
path
M l b; L wd8 vc; L l y1; L x2 y1; L x2 t; L x9 t; L x9 y1; L r y1; L x10 vc; L r b; L x9 b; L x9 y2; L x2 y2; L x2 b; Z
