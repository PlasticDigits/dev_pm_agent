// Minimal EFF short list subset for word-style codes (first 256 words)
const WORDS = [
  'able', 'acid', 'also', 'apex', 'arch', 'atom', 'aunt', 'away', 'axis', 'back',
  'ball', 'band', 'bank', 'base', 'bath', 'beam', 'bear', 'beat', 'been', 'bell',
  'belt', 'best', 'bill', 'bird', 'blue', 'boat', 'body', 'bolt', 'bomb', 'bond',
  'bone', 'book', 'boom', 'bore', 'born', 'boss', 'both', 'bowl', 'bulb', 'burn',
  'bush', 'bust', 'cafe', 'cage', 'cake', 'call', 'calm', 'came', 'camp', 'card',
  'care', 'cart', 'case', 'cash', 'cast', 'cell', 'chat', 'chip', 'city', 'club',
  'coal', 'coat', 'code', 'cold', 'comb', 'come', 'cook', 'cool', 'cope', 'copy',
  'cord', 'core', 'cost', 'crew', 'crop', 'crow', 'cube', 'cult', 'dare', 'dark',
  'data', 'date', 'dawn', 'days', 'dead', 'deal', 'dear', 'debt', 'deep', 'deny',
  'desk', 'diet', 'disc', 'disk', 'door', 'down', 'drag', 'draw', 'drop', 'drum',
  'duck', 'dude', 'dumb', 'dump', 'duck', 'each', 'earl', 'earn', 'ease', 'east',
  'easy', 'echo', 'edge', 'else', 'even', 'ever', 'evil', 'exam', 'exit', 'face',
  'fact', 'fail', 'fair', 'fall', 'fame', 'farm', 'fast', 'fate', 'fear', 'feed',
  'feel', 'feet', 'fell', 'felt', 'file', 'fill', 'film', 'find', 'fine', 'fire',
  'firm', 'fish', 'five', 'flat', 'flow', 'folk', 'food', 'foot', 'fork', 'form',
  'fort', 'four', 'free', 'from', 'fuel', 'full', 'fund', 'gain', 'game', 'gate',
  'gave', 'gear', 'gene', 'gift', 'girl', 'give', 'glad', 'glow', 'goal', 'goes',
  'gold', 'golf', 'gone', 'good', 'gray', 'grew', 'grey', 'grow', 'gulf', 'hair',
  'half', 'hall', 'hand', 'hang', 'hard', 'harm', 'hate', 'have', 'head', 'hear',
  'heat', 'held', 'help', 'hero', 'high', 'hill', 'hire', 'hold', 'hole', 'home',
  'hope', 'host', 'hour', 'huge', 'hunt', 'hurt', 'idea', 'inch', 'into', 'iron',
  'item', 'jack', 'jane', 'java', 'jazz', 'join', 'jump', 'jury', 'just', 'keen',
  'keep', 'kept', 'kick', 'kill', 'kind', 'king', 'knee', 'knew', 'know', 'lack',
  'lady', 'laid', 'lake', 'land', 'lane', 'last', 'late', 'lead', 'left', 'less',
  'life', 'lift', 'like', 'line', 'link', 'list', 'live', 'load', 'loan', 'lock',
  'long', 'look', 'lord', 'lose', 'loss', 'lost', 'love', 'luck', 'made', 'mail',
  'main', 'make', 'male', 'many', 'mark', 'mass', 'matt', 'meal', 'mean', 'meat',
  'meet', 'menu', 'mere', 'mike', 'mile', 'milk', 'mind', 'mine', 'miss', 'mode',
  'mood', 'moon', 'more', 'most', 'move', 'much', 'must', 'myth', 'name', 'navy',
  'near', 'neck', 'need', 'nest', 'news', 'next', 'nice', 'nine', 'none', 'nose',
  'note', 'okay', 'once', 'only', 'onto', 'open', 'oral', 'over', 'pace', 'pack',
  'page', 'paid', 'pain', 'pair', 'palm', 'park', 'part', 'pass', 'past', 'path',
  'peak', 'pick', 'pile', 'pink', 'pipe', 'plan', 'play', 'plot', 'plug', 'plus',
  'pool', 'poor', 'port', 'post', 'pull', 'pure', 'push', 'quit', 'race', 'rail',
];

export function generateWordCode(): string {
  const words: string[] = [];
  const arr = new Uint32Array(4);
  crypto.getRandomValues(arr);
  for (let i = 0; i < 4; i++) {
    words.push(WORDS[arr[i] % WORDS.length]);
  }
  return words.join('-');
}
