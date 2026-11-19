%% What a preview compile of Lily Studio includes before the score. It is
%% passed as -dinclude-settings, which takes one file, and it names the files
%% beside it, so that lilypond finds them here and not in the score's folder:
%% the playback map for the playhead (D26) and the words for Karaoke (D133).
\include "timing.ly"
\include "words.ly"
