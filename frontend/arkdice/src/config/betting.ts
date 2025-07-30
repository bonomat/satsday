export const BETTING_CONFIG = {
  MIN_BET_SATS: Number(import.meta.env.VITE_MIN_BET_SATS) || 500,
  MAX_BET_SATS: Number(import.meta.env.VITE_MAX_BET_SATS) || 500000,
};