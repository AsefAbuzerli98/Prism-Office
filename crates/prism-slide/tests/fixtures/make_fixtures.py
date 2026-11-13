"""Generates python-pptx.pptx (run: python3 make_fixtures.py out.pptx)."""
import sys, io, struct, zlib
from pptx import Presentation
from pptx.util import Inches, Pt, Emu
from pptx.dml.color import RGBColor
from pptx.enum.shapes import MSO_SHAPE
from pptx.chart.data import CategoryChartData
from pptx.enum.chart import XL_CHART_TYPE

def png(w, h, rgb):
    raw = b"".join(b"\x00" + bytes(rgb) * w for _ in range(h))
    def chunk(t, d):
        c = struct.pack(">I", len(d)) + t + d
        return c + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b"")

prs = Presentation()
prs.slide_width, prs.slide_height = Inches(13.333), Inches(7.5)
s = prs.slides.add_slide(prs.slide_layouts[0])
s.shapes.title.text = "Prism Office"
s.placeholders[1].text = "Unified 3-in-1 suite"
s.notes_slide.notes_text_frame.text = "Welcome everyone.\nIntroduce the team."

s = prs.slides.add_slide(prs.slide_layouts[1])
s.shapes.title.text = "Agenda"
tf = s.placeholders[1].text_frame
tf.text = "Documents"
for t, lvl in [("Tracked changes", 1), ("Workbooks", 0), ("Presentations", 0)]:
    p = tf.add_paragraph(); p.text = t; p.level = lvl

s = prs.slides.add_slide(prs.slide_layouts[6])
star = s.shapes.add_shape(MSO_SHAPE.STAR_5_POINT, Inches(1), Inches(1), Inches(2), Inches(2))
star.fill.solid(); star.fill.fore_color.rgb = RGBColor(0xFF, 0xC0, 0x00)
star.name = "Star"
box = s.shapes.add_shape(MSO_SHAPE.ROUNDED_RECTANGLE, Inches(4), Inches(1), Inches(3), Inches(1.5))
box.text_frame.text = "Rounded box"
box.rotation = 10
arrow = s.shapes.add_shape(MSO_SHAPE.RIGHT_ARROW, Inches(8), Inches(1), Inches(3), Inches(1))
pic = s.shapes.add_picture(io.BytesIO(png(40, 30, (40, 120, 200))), Inches(1), Inches(4), Inches(2), Inches(1.5))
pic.name = "Blue picture"
tbl = s.shapes.add_table(2, 3, Inches(4), Inches(4), Inches(5), Inches(1.2)).table
for r in range(2):
    for c in range(3):
        tbl.cell(r, c).text = f"R{r}C{c}"
grp = s.shapes.add_group_shape()
grp.shapes.add_shape(MSO_SHAPE.OVAL, Inches(10), Inches(4), Inches(1), Inches(1))
grp.shapes.add_shape(MSO_SHAPE.OVAL, Inches(11.2), Inches(4), Inches(1), Inches(1))
tb = s.shapes.add_textbox(Inches(1), Inches(6.2), Inches(6), Inches(0.6))
r = tb.text_frame.paragraphs[0].add_run(); r.text = "Visit prism.example"; r.hyperlink.address = "https://prism.example/"
r.font.size = Pt(20); r.font.bold = True

s = prs.slides.add_slide(prs.slide_layouts[5])
s.shapes.title.text = "Chart"
cd = CategoryChartData(); cd.categories = ["Q1", "Q2", "Q3"]; cd.add_series("Sales", (3, 5, 4))
s.shapes.add_chart(XL_CHART_TYPE.COLUMN_CLUSTERED, Inches(2), Inches(1.5), Inches(8), Inches(5), cd)
prs.save(sys.argv[1])
