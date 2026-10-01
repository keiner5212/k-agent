export const STORED_SECRET_MASK = "********";

export const storedSecretDisplay = (hasSecret: boolean | undefined): string =>
  hasSecret ? STORED_SECRET_MASK : "";

export const isStoredSecretMask = (value: string): boolean => value === STORED_SECRET_MASK;

export const clearStoredSecretMask = (value: string, onClear: () => void): void => {
  if (isStoredSecretMask(value)) onClear();
};
