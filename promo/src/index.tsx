import React from 'react';
import { Composition, registerRoot } from 'remotion';
import { Promo, TOTAL } from './Promo';
import { Short, SHORT_TOTAL } from './Short';

const Root: React.FC = () => (
  <>
    <Composition id="Promo" component={Promo} durationInFrames={TOTAL} fps={30} width={1920} height={1080} />
    <Composition id="Short" component={Short} durationInFrames={SHORT_TOTAL} fps={30} width={1080} height={1920} />
  </>
);

registerRoot(Root);
