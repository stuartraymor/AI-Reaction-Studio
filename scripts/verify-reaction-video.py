import json, subprocess, sys, struct, math
from pathlib import Path
import imageio_ffmpeg
from PIL import Image, ImageChops, ImageStat, ImageDraw

out=Path(sys.argv[1]).resolve()
ff=imageio_ffmpeg.get_ffmpeg_exe()
video=out/'reaction.mp4'
decoded=subprocess.run([ff,'-v','error','-xerror','-i',str(video),'-f','null','-'],capture_output=True,text=True)
(out/'decode-check.txt').write_text(decoded.stderr or 'Full audio/video decode passed.\n')
if decoded.returncode: raise RuntimeError(decoded.stderr)
p=json.loads((out/'provenance.json').read_text(encoding='utf-8-sig'))
hold=p['holdDurationTicks']/254016000000
times=[1,2.25,2+hold/2,2+hold+0.5]
frames=[]
for i,t in enumerate(times):
    dest=out/f'verification-{i}.png'
    subprocess.run([ff,'-v','error','-ss',str(t),'-i',str(video),'-frames:v','1','-y',str(dest)],check=True)
    frames.append(Image.open(dest).convert('RGB'))
box=(0,50,450,350)
diff=ImageStat.Stat(ImageChops.difference(frames[1].crop(box),frames[2].crop(box))).mean
sheet=Image.new('RGB',(1280,770),'#151921')
d=ImageDraw.Draw(sheet)
for i,(frame,t) in enumerate(zip(frames,times)):
    x=(i%2)*640;y=(i//2)*385
    sheet.paste(frame,(x,y+25));d.text((x+12,y+7),f'{t:.3f}s | '+('source' if i==0 else 'resumed' if i==3 else 'paused reaction'),fill='white')
sheet.save(out/'verification-contact-sheet.png')
resumed_diff=ImageStat.Stat(ImageChops.difference(frames[1].crop(box),frames[3].crop(box))).mean
timeline=json.loads((out/'timeline.json').read_text())
reopened=json.loads((out/'reopened-timeline.json').read_text())
original_audio=timeline['audio'][0]['items']
voice_clips=timeline['audio'][1]['items']
source_start=p['reactionStartTicks'];resume=source_start+p['holdDurationTicks']
source_silent=all(c['end']<=source_start or c['start']>=resume for c in original_audio)
voice_aligned=len(voice_clips)==1 and voice_clips[0]['start']==source_start and voice_clips[0]['end']<=resume
mouth_keys=timeline['video'][1]['items'][0]['effects'][-1]['params']['size']['keyframes']
wav=next((out/'assets').glob('*.wav')).read_bytes()
samples=[x[0] for x in struct.iter_unpack('<f',wav[44:])]
voice_finite=all(math.isfinite(x) for x in samples)
voice_rms=math.sqrt(sum(x*x for x in samples)/len(samples)) if samples else 0
reopen_equal=all(timeline.get(k)==reopened.get(k) for k in ['video','audio','duration','durationFrames'])
report={'fullDecodePassed':True,'pauseBackgroundMeanAbsoluteRgbDifference':diff,'pauseBackgroundStable':max(diff)<2,'resumedBackgroundChange':resumed_diff,'resumedMotion':max(resumed_diff)>1,'originalAudioAbsentDuringReaction':source_silent,'voiceStartsAtReactionAndFitsPause':voice_aligned,'mouthKeyframes':mouth_keys,'voiceSamplesFinite':voice_finite,'voiceRms':voice_rms,'editableProjectReopenEqual':reopen_equal,'frameTimesSeconds':times,'ffmpeg':subprocess.run([ff,'-version'],capture_output=True,text=True).stdout.splitlines()[0]}
(out/'verification.json').write_text(json.dumps(report,indent=2))
print(json.dumps(report,indent=2))
if not all([report['pauseBackgroundStable'],report['resumedMotion'],source_silent,voice_aligned,mouth_keys>1,voice_finite,voice_rms>0.001,reopen_equal]):raise RuntimeError('reaction acceptance check failed')
