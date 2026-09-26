import { resourceRows } from "./performanceDiagnostics";

it("reports resource wait, download and transfer cost slowest first", () => {
  const entry = (name:string,duration:number,requestStart:number,responseStart:number,responseEnd:number,transferSize:number,initiatorType:string) => ({name,duration,requestStart,responseStart,responseEnd,transferSize,initiatorType}) as PerformanceResourceTiming;
  expect(resourceRows([
    entry("https://reader.test/assets/app.js",40,2,12,42,2048,"script"),
    entry("https://reader.test/api/bootstrap?workspace=one",2500,5,2400,2505,4096,"fetch"),
  ])).toEqual([
    {resource:"/api/bootstrap?workspace=one",type:"fetch",durationMs:2500,waitMs:2395,downloadMs:105,transferKiB:4},
    {resource:"/assets/app.js",type:"script",durationMs:40,waitMs:10,downloadMs:30,transferKiB:2},
  ]);
});
