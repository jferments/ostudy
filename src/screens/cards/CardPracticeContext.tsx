import { createContext, type ReactNode, useContext } from "react";

export type PracticeCardsLauncher = (cardIds: string[]) => void | Promise<void>;

const CardPracticeContext = createContext<PracticeCardsLauncher | null>(null);

export function CardPracticeProvider({
  onPracticeCards,
  children,
}: {
  onPracticeCards: PracticeCardsLauncher;
  children: ReactNode;
}) {
  return (
    <CardPracticeContext.Provider value={onPracticeCards}>
      {children}
    </CardPracticeContext.Provider>
  );
}

const missingPracticeLauncher: PracticeCardsLauncher = () => {
  throw new Error("Card practice launcher is unavailable outside the Flashcards workspace.");
};

export function useCardPracticeLauncher(): PracticeCardsLauncher {
  return useContext(CardPracticeContext) ?? missingPracticeLauncher;
}
