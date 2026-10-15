import Foundation
import CoreText
import CoreGraphics
let font = CTFontCreateWithName("AvenirNext-DemiBold" as CFString, 200, nil)
let text = NSAttributedString(string: "Jacket", attributes: [NSAttributedString.Key(kCTFontAttributeName as String): font, NSAttributedString.Key(kCTKernAttributeName as String): -2.0])
let line = CTLineCreateWithAttributedString(text)
let combined = CGMutablePath()
for rawRun in CTLineGetGlyphRuns(line) as NSArray {
 let run = rawRun as! CTRun
 let count = CTRunGetGlyphCount(run)
 var glyphs = [CGGlyph](repeating: 0, count: count)
 var positions = [CGPoint](repeating: .zero, count: count)
 CTRunGetGlyphs(run, CFRange(location: 0, length: 0), &glyphs)
 CTRunGetPositions(run, CFRange(location: 0, length: 0), &positions)
 let attrs = CTRunGetAttributes(run) as NSDictionary
 let runFont = attrs[kCTFontAttributeName] as! CTFont
 for i in 0..<count {
  if let path = CTFontCreatePathForGlyph(runFont, glyphs[i], nil) {
   combined.addPath(path, transform: CGAffineTransform(translationX: positions[i].x, y: positions[i].y))
  }
 }
}
let bounds = combined.boundingBoxOfPath
var transform = CGAffineTransform(a: 1, b: 0, c: 0, d: -1, tx: -bounds.minX, ty: bounds.maxY)
let svgPath = combined.copy(using: &transform)!
func p(_ v: CGPoint) -> String { String(format: "%.3f %.3f", Double(v.x), Double(v.y)) }
var commands: [String] = []
svgPath.applyWithBlock { pointer in
 let e = pointer.pointee
 switch e.type {
 case .moveToPoint: commands.append("M" + p(e.points[0]))
 case .addLineToPoint: commands.append("L" + p(e.points[0]))
 case .addQuadCurveToPoint: commands.append("Q" + p(e.points[0]) + " " + p(e.points[1]))
 case .addCurveToPoint: commands.append("C" + p(e.points[0]) + " " + p(e.points[1]) + " " + p(e.points[2]))
 case .closeSubpath: commands.append("Z")
 @unknown default: break
 }
}
let result: [String: Any] = ["font": CTFontCopyPostScriptName(font) as String, "width": bounds.width, "height": bounds.height, "path": commands.joined(separator: " ")]
let data = try JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys])
FileHandle.standardOutput.write(data)
