export interface GameAddress {
  address: string;
  max_roll: number;
  multiplier: string;
  multiplier_value: number;
  win_probability: number;
  max_bet_amount: number;
}

export interface GameInfo {
  roll_range: string;
  win_condition: string;
}

export interface GameData {
  game_addresses: GameAddress[];
  info: GameInfo;
}
