import json,subprocess,sys,struct,math
from pathlib import Path
import imageio_ffmpeg
from PIL import Image,ImageChops,ImageStat,ImageDraw
out=Path(sys.argv[1]).resolve();ff=imageio_ffmpeg.get_ffmpeg_exe();video=out/'reaction.mp4';ticks=254016000000
r=subprocess.run([ff,'-v','error','-xerror','-i',str(video),'-f','null','-'],capture_output=True,text=True);assert r.returncode==0,r.stderr
subtitled=(out/'captions.json').exists();roi=(0,60,450,140) if subtitled else (0,60,450,350)
before=json.loads((out/'timeline.json').read_text());after=json.loads((out/'reopened-timeline.json').read_text());timing=json.loads((out/'reaction-timing.json').read_text()); checks=[];frames=[]
def frame(t, media=video):
 r=subprocess.run([ff,'-v','error','-ss',str(t),'-i',str(media),'-frames:v','1','-f','image2pipe','-vcodec','png','-'],capture_output=True,check=True)
 import io
 return Image.open(io.BytesIO(r.stdout)).convert('RGB')
for n,t in enumerate(timing):
 start=t['timelineStartTicks']/ticks;hold=t['holdDurationTicks']/ticks
 a=frame(start+min(.3,hold/4));b=frame(start+hold*.7);frames.extend([a,b]);diff=ImageStat.Stat(ImageChops.difference(a.crop(roi),b.crop(roi))).mean
 source=Path(json.loads((out/'analysis.json').read_text())['source']);actual=frame(start+hold+.125);expected=frame(t['snappedSourceTimestampTicks']/ticks+.125,source);resume_diff=ImageStat.Stat(ImageChops.difference(actual.crop((0,60,450,350)),expected.crop((0,60,450,350)))).mean
 assert max(resume_diff)<8,resume_diff
 silent=all(c['end']<=t['timelineStartTicks'] or c['start']>=t['timelineStartTicks']+t['holdDurationTicks'] for c in before['audio'][0]['items'])
 voice=before['audio'][1]['items'][n];aligned=voice['start']==t['timelineStartTicks'] and voice['end']<=t['timelineStartTicks']+t['holdDurationTicks']
 checks.append(dict(resumeMatchesSource=max(resume_diff)<8,resumeRgbDifference=resume_diff,backgroundStable=max(diff)<2,backgroundRgbDifference=diff,sourceAudioAbsent=silent,narrationAligned=aligned))
 assert max(diff)<2 and silent and aligned,checks
samples=[]
for p in (out/'assets').glob('*.wav'):
 values=[x[0] for x in struct.iter_unpack('<f',p.read_bytes()[44:])];assert values and all(math.isfinite(x) for x in values);rms=math.sqrt(sum(x*x for x in values)/len(values));assert rms>.001;samples.append(dict(file=p.name,rms=rms))
reopen=all(before.get(k)==after.get(k) for k in ['video','audio','duration','durationFrames']);assert reopen
sheet=Image.new('RGB',(1280,385*len(timing)),'#151921');d=ImageDraw.Draw(sheet)
for i,f in enumerate(frames):x=i%2*640;y=i//2*385;sheet.paste(f,(x,y+25));d.text((x+10,y+7),f'Reaction {i//2+1}: '+('early hold' if i%2==0 else 'late hold'),fill='white')
sheet.save(out/'verification-contact-sheet.png')
caption_report=None
if subtitled:
 captions=json.loads((out/'captions.json').read_text());assert captions==json.loads((out/'reopened-captions.json').read_text())
 cues=captions['tracks'][0]['captions'];assert cues
 for t in timing:
  start=t['timelineStartTicks'];end=start+t['speechDurationTicks'];matched=[c for c in cues if c['start']>=start and c['end']<=end]
  assert matched and all(c['end']>c['start'] for c in matched)
  assert ' '.join(' '.join(c['text'] for c in matched).split())==' '.join(t['commentary'].split())
  actual=frame(start/ticks+.1);expected=frame(t['snappedSourceTimestampTicks']/ticks,source)
  delta=ImageStat.Stat(ImageChops.difference(actual.crop((0,140,450,240)),expected.crop((0,140,450,240)))).mean
  assert max(delta)>1,('captions were not burned into export',delta)
 assert (out/'reaction.srt').stat().st_size>50
 caption_report=dict(cues=len(cues),textMatchesApprovedScript=True,cuesFitNarration=True,editableCaptionReopenEqual=True,srtExportPresent=True,burnedTextPixelsDetected=True,timing='estimated')
report=dict(commentaryCaptions=caption_report,fullAudioVideoDecodePassed=True,reactions=checks,narration=samples,editableProjectReopenEqual=reopen,durationSeconds=before['duration']/ticks)
(out/'verification.json').write_text(json.dumps(report,indent=2));print(json.dumps(report,indent=2))
