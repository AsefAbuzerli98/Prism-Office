import datetime, sys
from openpyxl import Workbook
from openpyxl.worksheet.table import Table, TableStyleInfo
from openpyxl.worksheet.datavalidation import DataValidation
from openpyxl.comments import Comment
from openpyxl.workbook.defined_name import DefinedName
out = sys.argv[1]
wb = Workbook()
ws = wb.active
ws.title = "Sales"
rows = [("Region","Product","Units","Price","Date"),
        ("East","Apple",10,1.5,datetime.date(2024,1,15)),
        ("West","Pear",20,2.25,datetime.date(2024,2,1)),
        ("East","Pear",30,2.0,datetime.date(2024,3,10)),
        ("North","Apple",40,1.75,datetime.date(2024,4,5))]
for r in rows: ws.append(r)
for i in range(2,6):
    ws[f"F{i}"] = f"=C{i}*D{i}"
    ws[f"E{i}"].number_format = "yyyy-mm-dd"
    ws[f"D{i}"].number_format = "#,##0.00"
ws["F1"] = "Revenue"
ws["H1"] = "Total"; ws["I1"] = "=SUM(F2:F5)"
ws["H2"] = "Lookup"; ws["I2"] = '=_xlfn.XLOOKUP("North",A2:A5,F2:F5)'
ws["H3"] = "Bool"; ws["I3"] = True
ws["H4"] = "Text"; ws["I4"] = "  spaced & <tagged>  "
ws["H5"] = "Named"; ws["I5"] = "=TaxRate*I1"
ws.column_dimensions["A"].width = 18
ws.column_dimensions["H"].width = 12
ws.row_dimensions[1].height = 24
ws.merge_cells("H7:I7"); ws["H7"] = "Merged"
ws.freeze_panes = "B2"
tab = Table(displayName="SalesTbl", ref="A1:F5"); tab.tableStyleInfo = TableStyleInfo(name="TableStyleMedium9", showRowStripes=True)
ws.add_table(tab)
dv = DataValidation(type="list", formula1='"East,West,North"'); ws.add_data_validation(dv); dv.add("A2:A5")
ws["A1"].comment = Comment("Region header", "Prism")
wb.defined_names["TaxRate"] = DefinedName("TaxRate", attr_text="Rates!$B$1")
r = wb.create_sheet("Rates"); r["A1"] = "Tax"; r["B1"] = 0.18; r["B1"].number_format = "0%"
wb.save(out)
